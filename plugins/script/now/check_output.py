#!/usr/bin/env python3
"""用 Python 的 datetime 独立核对 now 插件的 msime-pack run 输出，由 scripts/test-script-plugins.sh 在 check_run.py 之后调用。

expected.jsonl 只能说明输出和上次提交的一样；这里从夹具里的 `time` 独立算一遍，说明它是对的：

- 参数为空、夹具给了时间时恰好 5 行，依次是 `date.isoformat()`、中文日期加星期、`HH:MM`、日期和时间、`datetime.isoformat(timespec="minutes")`（带时区偏移），注释依次是「日期」「中文」「时间」「日期时间」「ISO 8601」；
- 夹具里的星期和日期本身对得上（夹具写错时这里先报出来）；
- 没给时间、或者参数不为空时没有任何行。

usage: check_output.py <fixture.jsonl> <granted.jsonl>
"""
import datetime
import json
import sys
from pathlib import Path

WEEKDAYS = "一二三四五六日"
NOTES = ["日期", "中文", "时间", "日期时间", "ISO 8601"]


def read(path: Path) -> list[dict]:
    return [json.loads(line) for line in path.read_text(encoding="utf-8").splitlines() if line.strip()]


def expected_rows(event: dict, problems: list[str]) -> list[list[str]]:
    time = event.get("time")
    if event.get("argument", "") or time is None:
        return []
    date = datetime.date(time["date"] // 10_000, time["date"] // 100 % 100, time["date"] % 100)
    if date.isoweekday() != time["weekday"]:
        problems.append(f"夹具的星期写错了：{date} 是星期 {date.isoweekday()}，夹具写 {time['weekday']}")
    zone = datetime.timezone(datetime.timedelta(minutes=time["utc_offset"]))
    moment = datetime.datetime.combine(date, datetime.time(time["minute"] // 60, time["minute"] % 60), zone)
    clock = moment.strftime("%H:%M")
    forms = [
        date.isoformat(),
        f"{date.year}年{date.month}月{date.day}日 星期{WEEKDAYS[date.isoweekday() - 1]}",
        clock,
        f"{date.isoformat()} {clock}",
        moment.isoformat(timespec="minutes"),
    ]
    return [[form, note] for form, note in zip(forms, NOTES)]


def main(argv: list[str]) -> int:
    if len(argv) != 2:
        raise SystemExit(__doc__.strip().splitlines()[-1])
    events, outputs = read(Path(argv[0])), read(Path(argv[1]))
    problems: list[str] = []
    if len(events) != len(outputs):
        problems.append(f"夹具有 {len(events)} 个事件，输出有 {len(outputs)} 行")
    for event, output in zip(events, outputs):
        want = expected_rows(event, problems)
        if output.get("rows") != want:
            problems.append(f"seq {output['seq']}：期望 {want}，实际 {output.get('rows')}")
    for problem in problems:
        print(f"FAIL now (datetime): {problem}")
    if not problems:
        print(f"ok now: {len(outputs)} events match Python datetime")
    return 1 if problems else 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
