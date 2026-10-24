"""手工可核算序列覆盖重放的年龄、内容变化和容量口径。"""

from replay import annotate, simulate


def put(key, count=1, digest='ordinary'):
    return ['S', '0', str(key), str(count), '8', str(count * 136), str(count + 1), digest]


def main():
    raw = [['N', '0', '128', '136']]
    for key in range(128):
        raw += [['G', '0', str(key), '0'], put(key)]
    # 覆盖键 0 不改年龄；下一次插入仍须淘汰它。
    raw += [put(0, 2, 'expanded'), ['G', '0', '128', '0'], put(128)]
    raw += [['P', '0', '0', '0'], ['G', '0', '0', '0'], put(0)]
    raw += [['C', '0'], ['D', '0']]
    events, actual = annotate(raw)
    baseline = simulate(events, 128)
    larger = simulate(events, 256)
    assert actual['external_insertions'] == 1
    assert baseline['counts']['G_misses'] == 130
    assert baseline['counts'].get('G_hits', 0) == 0
    assert baseline['counts'].get('changed_content_hits', 0) == 0
    assert baseline['peak_across_live_instances']['key_plus_page_requested_bytes'] == 128 * 144 + 136
    assert larger['counts']['G_misses'] == 129
    assert larger['counts']['G_hits'] == 1
    assert larger['counts']['changed_content_hits'] == 1
    assert larger['counts']['probe_outcome_changes'] == 1
    assert larger['peak_across_live_instances']['key_plus_page_requested_bytes'] == 129 * 144 + 136

    empty = [['N', '0', '128', '136']]
    for key in range(129):
        empty += [['G', '0', str(key), '0'], ['S', '0', str(key), '0', '8', '0', '0', 'empty']]
    empty += [['G', '0', '0', '0'], ['S', '0', '0', '0', '8', '0', '0', 'empty'], ['D', '0']]
    events, _ = annotate(empty)
    skipped = simulate(events, 128, skip_empty=True)
    assert skipped['counts']['G_misses'] == 130
    assert skipped['counts'].get('owned_key_allocations', 0) == 0
    assert skipped['peak_across_live_instances']['key_plus_page_requested_bytes'] == 0
    split = simulate(events, 64, 256)
    assert split['counts']['G_misses'] == 129
    assert split['counts']['G_hits'] == 1
    assert split['peak_across_live_instances']['key_plus_page_requested_bytes'] == 129 * 8

    invalid = raw.copy()
    invalid[-4] = ['P', '0', '0', '1']
    try:
        annotate(invalid)
    except AssertionError:
        pass
    else:
        raise AssertionError('不得接受不符合原 FIFO 的实际命中')
    print('年龄、清空、独立空页、内容变化、请求字节及错误轨迹拒绝检查通过')


if __name__ == '__main__':
    main()
