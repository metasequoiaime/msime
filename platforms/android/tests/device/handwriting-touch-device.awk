# 从 getevent -lp 的能力表选择触屏，不依赖旧版模拟器的设备名称。
function consider() {
    if (device !~ /^\/dev\/input\/event[0-9]+$/ || !slot || !tracking || !x || !y) return
    if (name == "virtio_input_multi_touch_1") primary = device
    else if (name !~ /^virtio_input_multi_touch_[0-9]+$/) {
        fallback = device
        count++
    }
}
{
    sub(/\r$/, "")
}
/^add device/ {
    consider()
    device = $NF
    name = ""
    slot = tracking = x = y = 0
}
/name:/ {
    name = $0
    sub(/^[^"]*"/, "", name)
    sub(/".*$/, "", name)
}
/ABS_MT_SLOT[[:space:]]*:/ { slot = 1 }
/ABS_MT_TRACKING_ID[[:space:]]*:/ { tracking = 1 }
/ABS_MT_POSITION_X[[:space:]]*:/ { x = 1 }
/ABS_MT_POSITION_Y[[:space:]]*:/ { y = 1 }
END {
    consider()
    if (primary != "") print primary
    else if (count == 1) print fallback
    else exit 1
}
