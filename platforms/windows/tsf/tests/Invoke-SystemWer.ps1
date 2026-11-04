[CmdletBinding()]
param(
  [Parameter(Mandatory = $true)][string]$BuildDirectory,
  [Parameter(Mandatory = $true)][string]$OutputDirectory,
  [string]$Configuration = 'RelWithDebInfo'
)
$ErrorActionPreference = 'Stop'
$payload = Join-Path (Resolve-Path -LiteralPath $BuildDirectory).Path "capture-test-payload/$Configuration"
$executable = (Resolve-Path -LiteralPath (Join-Path $payload 'msime-tsf-capture-system-test.exe')).Path
$helper = (Resolve-Path -LiteralPath (Join-Path $payload 'MetasequoiaImeCrashWer.dll')).Path
$output = [IO.Path]::GetFullPath($OutputDirectory)
if (Test-Path -LiteralPath $output) { throw '输出目录必须是尚不存在的合成专项目录' }
# GitHub Windows runner 可能让 WER 服务处于按需停止状态；运行时 helper 只有在服务真正接管故障后才会收到回调。
# 只启动服务，不改 WER 策略或注册表策略；服务本来已运行时保持原状。
$werService = Get-Service -Name WerSvc -ErrorAction SilentlyContinue
if ($werService -and $werService.Status -ne 'Running') {
  Start-Service -Name WerSvc
  $werService.WaitForStatus([System.ServiceProcess.ServiceControllerStatus]::Running, [TimeSpan]::FromSeconds(15))
}
# 只登记本次 helper 的完整路径，不改变系统通用 WER 策略或其他模块的值。
$image = [IO.File]::ReadAllBytes($executable)
$machine = [BitConverter]::ToUInt16($image, [BitConverter]::ToInt32($image, 0x3c) + 4)
$view = if ($machine -eq 0x14c) { [Microsoft.Win32.RegistryView]::Registry32 } else { [Microsoft.Win32.RegistryView]::Registry64 }
$registry = [Microsoft.Win32.RegistryKey]::OpenBaseKey([Microsoft.Win32.RegistryHive]::CurrentUser, $view)
$key = $registry.CreateSubKey('Software\Microsoft\Windows\Windows Error Reporting\RuntimeExceptionHelperModules')
if ($key.GetValueNames() -contains $helper) {
  $key.Dispose()
  $registry.Dispose()
  throw '测试路径已经在允许列表中，拒绝覆盖'
}
$process = $null
$stdout = $null
$stderr = $null
$caseDirectory = $null
function Save-CaseOutput {
  # 超时也保留已读取的诊断；只能取已完成任务，不能让失败清理再卡在管道上。
  if ($caseDirectory) {
    if ($stdout -and $stdout.Status -eq [Threading.Tasks.TaskStatus]::RanToCompletion) {
      [IO.File]::WriteAllText((Join-Path $caseDirectory 'stdout.txt'), $stdout.GetAwaiter().GetResult())
    }
    if ($stderr -and $stderr.Status -eq [Threading.Tasks.TaskStatus]::RanToCompletion) {
      [IO.File]::WriteAllText((Join-Path $caseDirectory 'stderr.txt'), $stderr.GetAwaiter().GetResult())
    }
  }
}
function Get-ThreadSnapshot {
  param([Diagnostics.Process]$Process)
  # 每个进程最多 128 个线程，只记 id、状态和等待原因。
  @($Process.Threads | Select-Object -First 128 | ForEach-Object {
    $entry = [ordered]@{ id = $_.Id }
    try {
      $state = $_.ThreadState
      $entry.state = $state.ToString()
      if ($state -eq [Diagnostics.ThreadState]::Wait) { $entry.wait_reason = $_.WaitReason.ToString() }
    } catch { $entry.snapshot_error = $_.Exception.GetType().Name }
    $entry
  })
}
function Save-SystemWerTimeout {
  param([Diagnostics.Process]$Target, [string]$Directory, [string]$Case)
  # 只保存本合成目标及为它工作的 WER worker 的有界状态；不采集内存、环境、命令行或其他应用的线程。
  $threads = @()
  $threadError = $null
  try {
    $threads = @(Get-ThreadSnapshot -Process $Target)
  } catch { $threadError = $_.Exception.GetType().Name }
  $workers = @()
  $workerError = $null
  try {
    $targetPattern = '(?:^|\s)-p\s+' + $Target.Id + '(?:\s|$)'
    $workers = @(Get-CimInstance Win32_Process -Filter "Name = 'WerFault.exe' OR Name = 'WerFaultSecure.exe'" -OperationTimeoutSec 3 |
      Where-Object { $_.ParentProcessId -eq $Target.Id -or $_.CommandLine -match $targetPattern } |
      Select-Object -First 16 | ForEach-Object {
        $workerId = $_.ProcessId
        $worker = [ordered]@{ id = $workerId; name = $_.Name; parent_id = $_.ParentProcessId }
        # worker 的线程等待原因用来区分下一次停顿是 WerFault 在等 WerSvc 还是在等别的；先取线程再取退出状态，受保护进程拒绝打开句柄时也能留下线程。
        try {
          $live = Get-Process -Id $workerId -ErrorAction Stop
          $worker.threads = @(Get-ThreadSnapshot -Process $live)
          $worker.has_exited = $live.HasExited
          $live.Dispose()
        } catch { $worker.snapshot_error = $_.Exception.GetType().Name }
        $worker
      })
  } catch { $workerError = $_.Exception.GetType().Name }
  # 记录落盘和系统退出分别判断，不能延长期限或把已有记录当成成功退出。
  $snapshot = [ordered]@{
    case = $Case
    target_pid = $Target.Id
    target_has_exited = $Target.HasExited
    completed_records = @(Get-ChildItem -LiteralPath $Directory -Filter '*.crash' -File | ForEach-Object Name)
    partial_records = @(Get-ChildItem -LiteralPath $Directory -Filter '*.werpartial' -File | ForEach-Object Name)
    target_threads = $threads
    thread_snapshot_error = $threadError
    related_wer_workers = $workers
    worker_snapshot_error = $workerError
  }
  $snapshot | ConvertTo-Json -Depth 5 | Set-Content -LiteralPath (Join-Path $Directory 'timeout.json') -Encoding UTF8
  $snapshot
}
function Test-SystemWerStall {
  param($Snapshot, [string]$Directory)
  # 只认 CI 上反复出现的两种 x86 系统侧停顿，二者都是目标仍在、恰好一个线程在等、其余线程全被挂起、没有半成品记录：
  # A 形：等待线程停在 WerSvc 的 LPC 回复上，找不到为它工作的 WerFault。
  # B 形：等待线程停在 LPC 回复或 UserRequest 上，唯一相关的 worker 是目标自己拉起的 WerFault.exe 且一直没结束目标；负向样本 withdrawn 也出现过，那时 helper 在看到未登记后立即返回，卡住的不是 helper。
  # WerFaultSecure、多个 worker、worker 不是目标的子进程都不认。负向样本出现记录、正向样本出现多条记录都说明 helper 越权，不能靠重跑掩盖。
  if ($Snapshot.target_has_exited -or $Snapshot.thread_snapshot_error -or $Snapshot.worker_snapshot_error) { return $false }
  if (@($Snapshot.partial_records).Count -ne 0) { return $false }
  $threads = @($Snapshot.target_threads)
  if ($threads.Count -eq 0) { return $false }
  foreach ($thread in $threads) {
    if ($thread.state -ne 'Wait' -or @('LpcReply', 'UserRequest', 'Suspended') -notcontains $thread.wait_reason) { return $false }
  }
  $waiting = @($threads | Where-Object { $_.wait_reason -ne 'Suspended' })
  if ($waiting.Count -ne 1) { return $false }
  $workers = @($Snapshot.related_wer_workers)
  if ($workers.Count -eq 0) {
    if ($waiting[0].wait_reason -ne 'LpcReply') { return $false }
  } elseif ($workers.Count -eq 1) {
    if ($workers[0].name -ne 'WerFault.exe' -or $workers[0].parent_id -ne $Snapshot.target_pid) { return $false }
  } else { return $false }
  $expectedPath = Join-Path $Directory 'expected.json'
  if (-not (Test-Path -LiteralPath $expectedPath -PathType Leaf)) { return $false }
  $expected = Get-Content -LiteralPath $expectedPath -Raw -Encoding UTF8 | ConvertFrom-Json
  $allowedRecords = if ($expected.record_expected) { 1 } else { 0 }
  return @($Snapshot.completed_records).Count -le $allowedRecords
}
try {
  New-Item -ItemType Directory -Path $output | Out-Null
  $key.SetValue($helper, 0, [Microsoft.Win32.RegistryValueKind]::DWord)
  foreach ($case in @('baseline', 'tip', 'host', 'external', 'withdrawn', 'reopened', 'released', 'caught')) {
    # 系统侧停顿时整例换新目录重跑一次，重跑仍须由系统退出并通过全部断言；首次的诊断留在原目录随 artifact 上传。
    for ($attempt = 1; ; $attempt++) {
      $caseDirectory = if ($attempt -eq 1) { Join-Path $output $case } else { Join-Path $output "$case-retry" }
      New-Item -ItemType Directory -Path $caseDirectory | Out-Null
      $stdout = $null
      $stderr = $null
      $process = New-Object System.Diagnostics.Process
      $process.StartInfo.FileName = $executable
      $process.StartInfo.Arguments = "$case `"$caseDirectory`""
      $process.StartInfo.UseShellExecute = $false
      $process.StartInfo.RedirectStandardOutput = $true
      $process.StartInfo.RedirectStandardError = $true
      if (-not $process.Start()) { throw "无法启动 $case" }
      # 夹具只写有界诊断；先开始读取，避免子进程管道互相等待。
      $stdout = $process.StandardOutput.ReadToEndAsync()
      $stderr = $process.StandardError.ReadToEndAsync()
      if ($process.WaitForExit(45000)) { break }
      $snapshot = Save-SystemWerTimeout -Target $process -Directory $caseDirectory -Case $case
      if ($attempt -ge 2 -or -not (Test-SystemWerStall -Snapshot $snapshot -Directory $caseDirectory)) {
        throw "$case 的系统 WER 投递超时"
      }
      $stalledWorkers = @($snapshot.related_wer_workers)
      $shape = if ($stalledWorkers.Count -eq 0) { '目标进程停在 WerSvc 的 LPC 回复上且没有 WER worker' } else { "目标进程拉起的 WerFault（pid $($stalledWorkers[0].id)）一直没有结束它" }
      Write-Warning "$case 的$shape，诊断见 $caseDirectory/timeout.json；结束卡住的进程后换新目录重跑一次"
      # 先结束卡住的 WerFault 再结束目标，两者都等到退出，旧 worker 不会在重跑期间继续持有目标或写原目录。按名字复核，避免 pid 已被复用时误杀。
      $liveWorkers = @($stalledWorkers | ForEach-Object { Get-Process -Id $_.id -ErrorAction SilentlyContinue } | Where-Object { $_.ProcessName -eq 'WerFault' })
      foreach ($worker in $liveWorkers) { $worker.Kill() }
      $process.Kill()
      foreach ($worker in $liveWorkers) {
        $worker.WaitForExit()
        $worker.Dispose()
      }
      $process.WaitForExit()
      Save-CaseOutput
      $process.Dispose()
      $process = $null
    }
    $process.WaitForExit()
    [IO.File]::WriteAllText((Join-Path $caseDirectory 'stdout.txt'), $stdout.GetAwaiter().GetResult())
    [IO.File]::WriteAllText((Join-Path $caseDirectory 'stderr.txt'), $stderr.GetAwaiter().GetResult())
    $expectedExit = if ($case -eq 'caught') { 0 } else { -1073741819 }
    if ($process.ExitCode -ne $expectedExit) { throw "$case 退出码 $($process.ExitCode)，预期 $expectedExit" }
    $expected = Get-Content -LiteralPath (Join-Path $caseDirectory 'expected.json') -Raw -Encoding UTF8 | ConvertFrom-Json
    Write-Output "$case error mode: inherited=$($expected.inherited_error_mode); effective=$($expected.effective_error_mode)"
    # WER 在目标进程退出后由独立 worker 投递；WaitForExit 只等待目标进程，不能代表 helper 已经运行完。
    # 给正、负两类样本都留出稳定窗口，避免把异步投递误判成缺失或在断言后才发现越权记录。
    $deadline = (Get-Date).AddSeconds(30)
    do {
      $records = @(Get-ChildItem -LiteralPath $caseDirectory -Filter '*.crash' -File)
      $partials = @(Get-ChildItem -LiteralPath $caseDirectory -Filter '*.werpartial' -File)
      $settled = if ($expected.record_expected) {
        $records.Count -eq 1 -and (Test-Path -LiteralPath $expected.path -PathType Leaf)
      } else {
        $records.Count -gt 0 -or $partials.Count -gt 0
      }
      if ($settled) { break }
      Start-Sleep -Milliseconds 250
    } while ((Get-Date) -lt $deadline)
    $records = @(Get-ChildItem -LiteralPath $caseDirectory -Filter '*.crash' -File)
    if ($expected.record_expected) {
      if ($records.Count -ne 1 -or -not (Test-Path -LiteralPath $expected.path -PathType Leaf)) {
        $files = @(Get-ChildItem -LiteralPath $caseDirectory -File | ForEach-Object Name) -join ', '
        throw "$case 未收到唯一的系统 WER 记录；预期路径=$($expected.path)；当前文件=$files"
      }
      $record = [IO.File]::ReadAllText($expected.path)
      $pattern = '^' + [regex]::Escape($expected.header) + 'EXCEPTION_ACCESS_VIOLATION \(0xc0000005\)\n' + [regex]::Escape($expected.module) + '\+0x[0-9a-f]+\n$'
      if ($record -cnotmatch $pattern) { throw "$case 的原头、异常码或自有模块偏移不符" }
      if ($case -eq 'reopened' -and ((Test-Path -LiteralPath $expected.initial_path) -or $expected.path -eq $expected.initial_path)) { throw '重开后错误复用了旧记录身份' }
    } elseif ($records.Count -ne 0) { throw "$case 错误采集了外部、撤回或已捕获的异常" }
    if (@(Get-ChildItem -LiteralPath $caseDirectory -Filter '*.werpartial' -File).Count -ne 0) { throw "$case 留下了未完成的记录" }
    Write-Output "PASS system WER $case; exit=$($process.ExitCode); records=$($records.Count)"
    $process.Dispose()
    $process = $null
  }
} finally {
  if ($process) {
    if (-not $process.HasExited) { $process.Kill(); $process.WaitForExit() }
    Save-CaseOutput
    $process.Dispose()
  }
  $key.DeleteValue($helper, $false)
  $remaining = $key.GetValueNames() -contains $helper
  $key.Dispose()
  $registry.Dispose()
  if ($remaining) { throw '测试 helper 允许列表清理失败' }
}
Write-Output 'PASS 系统投递专项及测试路径允许列表清理；未调用产品上传入口'
