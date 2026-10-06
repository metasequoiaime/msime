# programs.msime 的虚拟机测试：用户单元经 systemd.packages 注册、socket 随登录前的用户管理器就位，
# 连接时 systemd 拉起 provider，语音服务的解释器带着豆包要的 websockets；设置窗口在 X 会话里打得开。
{ pkgs, module }:
pkgs.testers.runNixOSTest {
  name = "msime-module";

  nodes.machine = {
    # 自动登录 alice 的 X 会话（IceWM），设置窗口在里面打开。
    imports = [
      module
      "${pkgs.path}/nixos/tests/common/x11.nix"
    ];
    test-support.displayManager.auto.user = "alice";
    # 测试驱动以 root 列出 X 窗口（wait_for_window），要用 alice 会话的授权。
    environment.variables.XAUTHORITY = "/home/alice/.Xauthority";
    # 语音服务的配置测试请求：不碰网络和录音，服务在 socket 上应答就说明启动走完了。
    environment.etc."msime-test/credential-test.json".text = ''
      {"version":1,"kind":"credential_test","query":{}}
    '';
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

    # 不认得的配置测试答 ok: false，有答复就说明服务进了 serve_forever，没有以状态 2 退出。
    reply = machine.succeed(
        "su alice -c 'timeout 30 nc -U /run/user/1000/msime-client/voice.sock'"
        " < /etc/msime-test/credential-test.json"
    )
    assert '"ok": false' in reply, reply
    for provider in ("online", "voice"):
        assert machine.get_unit_info(f"msime-linux-{provider}.service", "alice")["ActiveState"] == "active"

    # 服务经 msime-linux-provider-session 按路径 exec 语音服务脚本，用的就是它 shebang 里的解释器；
    # 拿这个解释器调用 websocket_dependency()，websockets 不可用时它抛异常。
    machine.succeed(
        "provider=$(readlink -f /run/current-system/sw/bin/msime-linux-voice-provider);"
        " PYTHONPATH=$(dirname $provider) $(sed -n '1s/^#!//p' $provider)"
        " -c 'import msime_voice_doubao; msime_voice_doubao.websocket_dependency()'"
    )

    # 剪贴板监视器挂在图形会话上，随自动登录的 X 会话由 graphical-session.target 拉起。
    clipboard = machine.get_unit_info("msime-linux-clipboard.service", "alice")
    assert "graphical-session.target" in clipboard["WantedBy"], clipboard["WantedBy"]
    machine.wait_for_x()
    machine.wait_for_unit("msime-linux-clipboard.service", "alice")

    # 设置窗口：从 PATH 上的 msime-linux-settings 起，与插件菜单走的是同一个命令。窗口出现、WebKit 的
    # 网页进程起来，说明 GTK 与 WebKit 的运行环境齐全；截图留在测试输出里，可以看页面是否渲染出来。
    machine.succeed("su - alice -c 'DISPLAY=:0 msime-linux-settings >/tmp/settings.log 2>&1 &'")
    machine.wait_for_window("水杉输入法", timeout=120)
    machine.wait_until_succeeds("pgrep -u alice -f WebKitWebProcess", timeout=60)
    machine.sleep(5)
    machine.screenshot("settings-window")
  '';
}
