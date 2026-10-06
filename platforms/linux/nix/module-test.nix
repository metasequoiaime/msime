# programs.msime 的虚拟机测试：用户单元经 systemd.packages 注册、socket 随登录前的用户管理器就位，
# 连接时 systemd 拉起 provider，语音服务的解释器带着豆包要的 websockets。
{ pkgs, module }:
pkgs.testers.runNixOSTest {
  name = "msime-module";

  nodes.machine = {
    imports = [ module ];
    programs.msime.enable = true;
    # linger 让用户管理器开机就起来，不必真的登录。
    users.users.alice = {
      isNormalUser = true;
      linger = true;
    };
  };

  testScript = ''
    machine.wait_for_unit("user@1000.service")
    for provider in ("online", "voice"):
        machine.wait_for_unit(f"msime-linux-{provider}.socket", "alice")
        # 连上 socket 就会触发激活。以 alice 连：provider 用 SO_PEERCRED 只接受同一用户。
        machine.succeed(f"su alice -c 'nc -zU /run/user/1000/msime-client/{provider}.sock'")
        machine.wait_for_unit(f"msime-linux-{provider}.service", "alice")

    # 语音服务在打开 socket 之前先查录音工具，虚拟机里没有，这一条记下来就说明启动走完了，没有以状态 2 退出。
    # 不用 journalctl --user-unit：它还要求 _UID 等于调用者，测试驱动以 root 运行，看不到 alice 的条目；
    # grep 不加 -q：测试驱动带 pipefail，grep -q 提前退出会让 journalctl 收到 SIGPIPE。
    machine.wait_until_succeeds(
        "journalctl _SYSTEMD_USER_UNIT=msime-linux-voice.service -o cat | grep 'recording unavailable'",
        timeout=60,
    )
    for provider in ("online", "voice"):
        assert machine.get_unit_info(f"msime-linux-{provider}.service", "alice")["ActiveState"] == "active"

    # 服务经 msime-linux-provider-session 按路径 exec 语音服务脚本，用的就是它 shebang 里的解释器；
    # 拿这个解释器调用 websocket_dependency()，websockets 不可用时它抛异常。
    machine.succeed(
        "provider=$(readlink -f /run/current-system/sw/bin/msime-linux-voice-provider);"
        " PYTHONPATH=$(dirname $provider) $(sed -n '1s/^#!//p' $provider)"
        " -c 'import msime_voice_doubao; msime_voice_doubao.websocket_dependency()'"
    )

    # 剪贴板监视器挂在图形会话上，虚拟机里没有图形会话，只核对它由 graphical-session.target 拉起。
    clipboard = machine.get_unit_info("msime-linux-clipboard.service", "alice")
    assert "graphical-session.target" in clipboard["WantedBy"], clipboard["WantedBy"]
  '';
}
