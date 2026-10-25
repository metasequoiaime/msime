# 仅创建并销毁合成桌面与隐藏窗口；不切换输入桌面，不操作真实输入法。
$ErrorActionPreference = 'Stop'
$ProgressPreference = 'SilentlyContinue'
Add-Type -TypeDefinition @'
using System;
using System.Collections.Generic;
using System.ComponentModel;
using System.Runtime.InteropServices;
using System.Text;
using System.Threading;

public static class MsimeDesktop9439Probe {
    [DllImport("user32.dll", CharSet=CharSet.Unicode, SetLastError=true)]
    static extern IntPtr CreateDesktop(string name, IntPtr device, IntPtr mode, uint flags, uint access, IntPtr security);
    [DllImport("user32.dll", SetLastError=true)] static extern bool CloseDesktop(IntPtr desktop);
    [DllImport("user32.dll", SetLastError=true)] static extern bool SetThreadDesktop(IntPtr desktop);
    [DllImport("user32.dll", SetLastError=true)] static extern IntPtr GetThreadDesktop(uint thread);
    [DllImport("kernel32.dll")] static extern uint GetCurrentThreadId();
    [DllImport("user32.dll", SetLastError=true)] static extern uint GetWindowThreadProcessId(IntPtr window, out uint pid);
    [DllImport("kernel32.dll", SetLastError=true)] static extern IntPtr OpenThread(uint access, bool inherit, uint tid);
    [DllImport("kernel32.dll", SetLastError=true)] static extern uint GetProcessIdOfThread(IntPtr thread);
    [DllImport("kernelbase.dll", SetLastError=true)] static extern bool CompareObjectHandles(IntPtr first, IntPtr second);
    [DllImport("user32.dll", CharSet=CharSet.Unicode, SetLastError=true)]
    static extern IntPtr OpenDesktop(string name, uint flags, bool inherit, uint access);
    [DllImport("user32.dll", CharSet=CharSet.Unicode, SetLastError=true)]
    static extern bool GetUserObjectInformation(IntPtr handle, int index, StringBuilder info, uint bytes, out uint needed);
    [DllImport("user32.dll", CharSet=CharSet.Unicode, SetLastError=true)]
    static extern IntPtr CreateWindowEx(uint ex, string cls, string title, uint style, int x, int y, int w, int h,
        IntPtr parent, IntPtr menu, IntPtr instance, IntPtr parameter);
    [DllImport("user32.dll", SetLastError=true)] static extern bool DestroyWindow(IntPtr window);
    delegate bool EnumWindow(IntPtr window, IntPtr data);
    [DllImport("user32.dll", SetLastError=true)] static extern bool EnumDesktopWindows(IntPtr desktop, EnumWindow callback, IntPtr data);
    [DllImport("kernel32.dll", CharSet=CharSet.Unicode, SetLastError=true)]
    static extern IntPtr CreateMutex(IntPtr security, bool owner, string name);
    [DllImport("kernel32.dll")] static extern bool CloseHandle(IntPtr handle);
    static void Check(bool condition, string message) {
        if (!condition) throw new InvalidOperationException(message + " (Win32 " + Marshal.GetLastWin32Error() + ")");
    }
    static string Name(IntPtr handle) {
        uint needed;
        StringBuilder text = new StringBuilder(1024);
        Check(GetUserObjectInformation(handle, 2, text, 2048, out needed), "object name query failed");
        return text.ToString();
    }
    static HashSet<IntPtr> Windows(IntPtr desktop) {
        HashSet<IntPtr> result = new HashSet<IntPtr>();
        EnumWindow callback = delegate(IntPtr window, IntPtr unused) { result.Add(window); return true; };
        Check(EnumDesktopWindows(desktop, callback, IntPtr.Zero), "desktop window enumeration failed");
        GC.KeepAlive(callback);
        return result;
    }
    public static string RunCrossProcess(string executable) {
        string token="MSIME9439Child"+Guid.NewGuid().ToString("N");
        IntPtr desktop=CreateDesktop(token,IntPtr.Zero,IntPtr.Zero,0,0xC3,IntPtr.Zero);
        Check(desktop!=IntPtr.Zero,"child desktop creation failed");
        System.Diagnostics.Process child=null;
        try {
            System.Diagnostics.ProcessStartInfo start=new System.Diagnostics.ProcessStartInfo(executable,token);
            start.UseShellExecute=false;
            start.CreateNoWindow=true;
            start.RedirectStandardInput=true;
            start.RedirectStandardOutput=true;
            start.RedirectStandardError=true;
            child=System.Diagnostics.Process.Start(start);
            Check(child!=null,"child process creation failed");
            var line=System.Threading.Tasks.Task.Factory.StartNew(delegate {return child.StandardOutput.ReadLine();});
            Check(line.Wait(10000),"child window readiness timeout");
            string[] values=(line.Result??"").Split(':');
            Check(values.Length==3,"child window descriptor unavailable");
            uint claimedPid=UInt32.Parse(values[0]);
            uint claimedThread=UInt32.Parse(values[1]);
            IntPtr window=new IntPtr(Int64.Parse(values[2]));
            // 另一个桌面的 HWND 在当前线程未必可查询，TID 归属必须从系统线程对象核对。
            uint windowPid;
            bool windowUsable=GetWindowThreadProcessId(window,out windowPid)!=0;
            IntPtr thread=OpenThread(0x0800,false,claimedThread);
            Check(thread!=IntPtr.Zero,"cross-process thread unavailable");
            try {
                uint actualPid=GetProcessIdOfThread(thread);
                Check(actualPid==(uint)child.Id && actualPid==claimedPid,"cross-process thread identity mismatch");
                IntPtr assigned=GetThreadDesktop(claimedThread);
                Check(Name(assigned)==token,"cross-process desktop identity mismatch");
                IntPtr opened=OpenDesktop(token,0,false,0xC3);
                Check(opened!=IntPtr.Zero,"cross-process desktop reopening failed");
                try {Check(CompareObjectHandles(assigned,opened),"desktop object identity mismatch");}
                finally {CloseDesktop(opened);}
            } finally {CloseHandle(thread);}
            Check(Windows(desktop).Contains(window),"child window missing from its desktop");
            child.StandardInput.WriteLine("stop");
            child.StandardInput.Close();
            Check(child.WaitForExit(10000),"child process shutdown timeout");
            Check(child.ExitCode==0,"child window cleanup failed");
            return "{\"crossProcessDesktopResolved\":true,\"threadProcessVerified\":true,\"desktopObjectVerified\":true,\"crossDesktopWindowUsable\":"+
                (windowUsable ? "true" : "false")+",\"switchedInputDesktop\":false}";
        } finally {
            if(child!=null) {
                if(!child.HasExited) {child.Kill();child.WaitForExit(5000);}
                child.Dispose();
            }
            Check(CloseDesktop(desktop),"child desktop cleanup failed");
        }
    }
    public static string Run() {
        string token = "MSIME9439Probe" + Guid.NewGuid().ToString("N");
        string mutexName = "Local\\" + token;
        IntPtr[] desktops = new IntPtr[2];
        IntPtr[] windows = new IntPtr[2];
        Thread[] threads = new Thread[2];
        ManualResetEvent[] ready = {new ManualResetEvent(false),new ManualResetEvent(false)};
        ManualResetEvent release = new ManualResetEvent(false);
        Exception[] errors = new Exception[2];
        bool[] sameMutex = new bool[2];
        bool[] ownMutex = new bool[2];
        string[] names = new string[2];
        IntPtr shared = CreateMutex(IntPtr.Zero, false, mutexName);
        Check(shared != IntPtr.Zero, "probe mutex unavailable");
        try {
            for (int i=0; i<2; i++) {
                desktops[i] = CreateDesktop(token + i, IntPtr.Zero, IntPtr.Zero, 0, 0xC3, IntPtr.Zero);
                Check(desktops[i] != IntPtr.Zero, "private desktop creation failed");
                int index=i;
                threads[i] = new Thread(delegate() {
                    IntPtr original = GetThreadDesktop(GetCurrentThreadId());
                    IntPtr common = IntPtr.Zero;
                    IntPtr scoped = IntPtr.Zero;
                    try {
                        Check(SetThreadDesktop(desktops[index]), "desktop binding failed before window creation");
                        names[index] = Name(GetThreadDesktop(GetCurrentThreadId()));
                        windows[index] = CreateWindowEx(0,"STATIC","synthetic probe",0x80000000,0,0,120,60,
                            IntPtr.Zero,IntPtr.Zero,IntPtr.Zero,IntPtr.Zero);
                        Check(windows[index] != IntPtr.Zero, "private desktop window creation failed");
                        common=CreateMutex(IntPtr.Zero,false,mutexName);
                        int commonError=Marshal.GetLastWin32Error();
                        sameMutex[index]=common!=IntPtr.Zero && commonError==183;
                        scoped=CreateMutex(IntPtr.Zero,false,mutexName+"."+index);
                        int scopedError=Marshal.GetLastWin32Error();
                        ownMutex[index]=scoped!=IntPtr.Zero && scopedError!=183;
                        ready[index].Set();
                        release.WaitOne(15000);
                    } catch(Exception error) { errors[index]=error; ready[index].Set(); }
                    finally {
                        if(windows[index]!=IntPtr.Zero) DestroyWindow(windows[index]);
                        if(common!=IntPtr.Zero) CloseHandle(common);
                        if(scoped!=IntPtr.Zero) CloseHandle(scoped);
                        SetThreadDesktop(original);
                    }
                });
                threads[i].IsBackground=true;
                threads[i].Start();
            }
            for(int i=0;i<2;i++) {
                Check(ready[i].WaitOne(10000),"desktop worker timeout");
                if(errors[i]!=null) throw errors[i];
                Check(names[i]==token+i,"worker attached to wrong desktop");
            }
            for(int i=0;i<2;i++) {
                uint process;
                uint thread=GetWindowThreadProcessId(windows[i],out process);
                Check(thread!=0 && process==(uint)System.Diagnostics.Process.GetCurrentProcess().Id,"window owner query failed");
                Check(Name(GetThreadDesktop(thread))==names[i],"main thread cannot resolve another desktop worker");
            }
            HashSet<IntPtr> first=Windows(desktops[0]);
            HashSet<IntPtr> second=Windows(desktops[1]);
            Check(first.Contains(windows[0])&&!first.Contains(windows[1]),"first desktop contains wrong window");
            Check(second.Contains(windows[1])&&!second.Contains(windows[0]),"second desktop contains wrong window");
            Check(sameMutex[0]&&sameMutex[1],"unscoped instance mutex does not collide as expected");
            Check(ownMutex[0]&&ownMutex[1],"desktop-scoped mutexes collide");
            return "{\"ownerThreadDesktopResolvedFromMain\":true,\"separateDesktopWindows\":true,\"unscopedMutexCollidesAcrossDesktops\":true,\"scopedMutexesIndependent\":true,\"switchedInputDesktop\":false,\"productionInputMethodChanged\":false}";
        } finally {
            release.Set();
            for(int i=0;i<2;i++) if(threads[i]!=null) Check(threads[i].Join(5000),"desktop worker cleanup timeout");
            for(int i=0;i<2;i++) {
                if(desktops[i]!=IntPtr.Zero) Check(CloseDesktop(desktops[i]),"private desktop cleanup failed");
                ready[i].Dispose();
            }
            CloseHandle(shared);
            release.Dispose();
        }
    }
}
'@
[MsimeDesktop9439Probe]::Run()

# 子进程独占自己的窗口线程，父进程仅依据系统返回的 PID、TID 和桌面归属核对。
$childSource = @'
using System;
using System.Diagnostics;
using System.Runtime.InteropServices;
public static class MsimeDesktop9439Child {
    [DllImport("user32.dll",CharSet=CharSet.Unicode,SetLastError=true)]
    static extern IntPtr OpenDesktop(string name,uint flags,bool inherit,uint access);
    [DllImport("user32.dll",SetLastError=true)] static extern bool CloseDesktop(IntPtr desktop);
    [DllImport("user32.dll",SetLastError=true)] static extern bool SetThreadDesktop(IntPtr desktop);
    [DllImport("user32.dll")] static extern IntPtr GetThreadDesktop(uint thread);
    [DllImport("kernel32.dll")] static extern uint GetCurrentThreadId();
    [DllImport("user32.dll",CharSet=CharSet.Unicode,SetLastError=true)]
    static extern IntPtr CreateWindowEx(uint ex,string cls,string title,uint style,int x,int y,int w,int h,
        IntPtr parent,IntPtr menu,IntPtr instance,IntPtr parameter);
    [DllImport("user32.dll",SetLastError=true)] static extern bool DestroyWindow(IntPtr window);
    public static int Main(string[] arguments) {
        if(arguments.Length!=1) return 2;
        IntPtr original=GetThreadDesktop(GetCurrentThreadId());
        IntPtr desktop=OpenDesktop(arguments[0],0,false,0xC3);
        if(desktop==IntPtr.Zero) return 3;
        IntPtr window=IntPtr.Zero;
        try {
            if(!SetThreadDesktop(desktop)) return 4;
            window=CreateWindowEx(0,"STATIC","synthetic child",0x80000000,0,0,120,60,
                IntPtr.Zero,IntPtr.Zero,IntPtr.Zero,IntPtr.Zero);
            if(window==IntPtr.Zero) return 5;
            Console.WriteLine(Process.GetCurrentProcess().Id+":"+GetCurrentThreadId()+":"+window.ToInt64());
            Console.Out.Flush();
            return Console.ReadLine()=="stop" ? 0 : 6;
        } finally {
            if(window!=IntPtr.Zero) DestroyWindow(window);
            SetThreadDesktop(original);
            CloseDesktop(desktop);
        }
    }
}
'@
$scratch = Join-Path ([IO.Path]::GetTempPath()) ('msime9439-' + [Guid]::NewGuid().ToString('N'))
[IO.Directory]::CreateDirectory($scratch) | Out-Null
$compiler = New-Object Microsoft.CSharp.CSharpCodeProvider
try {
    $parameters = New-Object System.CodeDom.Compiler.CompilerParameters
    $parameters.GenerateExecutable = $true
    $parameters.GenerateInMemory = $false
    $parameters.OutputAssembly = Join-Path $scratch 'desktop-child.exe'
    $parameters.ReferencedAssemblies.Add('System.dll') | Out-Null
    $parameters.ReferencedAssemblies.Add('System.Core.dll') | Out-Null
    $result = $compiler.CompileAssemblyFromSource($parameters, $childSource)
    if ($result.Errors.HasErrors) { throw '合成桌面子进程编译失败' }
    [MsimeDesktop9439Probe]::RunCrossProcess($parameters.OutputAssembly)
} finally {
    $compiler.Dispose()
    Remove-Item -LiteralPath $scratch -Recurse -Force
}
