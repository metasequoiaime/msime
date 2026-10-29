//! 模拟用户怎么从候选里挑：纯函数，输入是这一刻的整张候选表（宿主触屏候选栏就是 `all_candidates` 的顺序）和这一块还没打出来的金标准，输出点哪一个。
//!
//! 用户只认文字是剩余金标准开头的候选，在候选栏露出的前 `strip` 个里挑盖住最多字的那个，一样多时挑靠前的。只在第一个就是金标准开头、而且后面没有更长的时才点第一个，所以把一个词拆成几个单字放在前面换不来首选率：整块就在候选栏里时用户直接点整块。

/// 用户要看几个候选、怎么翻。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Policy {
    /// 候选栏不滑动就看得到的个数。
    pub strip: usize,
    /// 展开面板每页几个。
    pub panel_page: usize,
    /// 展开面板最多翻到第几页（含第一页）；0 表示不开面板。
    pub panel_pages: usize,
}

/// 用户这一步做什么。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Decision {
    /// 在候选栏上点第 `seat` 个（整张表里的下标），它盖住剩余金标准的前 `chars` 个字。
    Strip { seat: usize, chars: usize },
    /// 打开面板，翻到第 `page` 页（从 0 起），点第 `seat` 个。
    Panel {
        seat: usize,
        chars: usize,
        page: usize,
    },
    /// 看过的范围里都没有。
    NotFound,
}

/// 文字是剩余金标准的非空前缀时返回它盖住几个字。
fn covers(candidate: &str, remaining: &str) -> Option<usize> {
    (!candidate.is_empty() && remaining.starts_with(candidate)).then(|| candidate.chars().count())
}

/// `seats` 范围里盖住最多字的候选，一样多取靠前的。
fn longest(
    candidates: &[&str],
    remaining: &str,
    seats: std::ops::Range<usize>,
) -> Option<(usize, usize)> {
    let mut best: Option<(usize, usize)> = None;
    for seat in seats {
        let Some(text) = candidates.get(seat) else {
            break;
        };
        if let Some(chars) = covers(text, remaining) {
            if best.is_none_or(|(_, longest)| chars > longest) {
                best = Some((seat, chars));
            }
        }
    }
    best
}

pub fn decide(candidates: &[&str], remaining: &str, policy: Policy) -> Decision {
    if let Some((seat, chars)) = longest(candidates, remaining, 0..policy.strip) {
        return Decision::Strip { seat, chars };
    }
    let page = policy.panel_page.max(1);
    for index in 0..policy.panel_pages {
        let start = index * page;
        if start >= candidates.len() {
            break;
        }
        if let Some((seat, chars)) = longest(candidates, remaining, start..start + page) {
            return Decision::Panel {
                seat,
                chars,
                page: index,
            };
        }
    }
    Decision::NotFound
}

/// 手滑点错时点的候选：候选栏上第一个之后、文字不是金标准开头的那一个。没有这样的候选时不点。
pub fn slip(candidates: &[&str], remaining: &str, policy: Policy) -> Option<usize> {
    (1..policy.strip.min(candidates.len()))
        .find(|&seat| !candidates[seat].is_empty() && covers(candidates[seat], remaining).is_none())
}

#[cfg(test)]
mod tests {
    use super::*;

    const POLICY: Policy = Policy {
        strip: 6,
        panel_page: 24,
        panel_pages: 2,
    };

    #[test]
    fn the_first_candidate_is_taken_when_nothing_longer_is_on_the_strip() {
        assert_eq!(
            decide(&["吃饭", "迟饭", "吃"], "吃饭", POLICY),
            Decision::Strip { seat: 0, chars: 2 }
        );
        // 第一个只是开头，候选栏上没有更长的：先点它，余下的再选。
        assert_eq!(
            decide(&["吃", "迟", "持"], "吃饭了", POLICY),
            Decision::Strip { seat: 0, chars: 1 }
        );
    }

    #[test]
    fn a_longer_match_further_along_the_strip_wins_over_the_first() {
        // 整块在第 3 个：拆成单字放在前面换不来首选。
        assert_eq!(
            decide(&["我", "哦", "我们走"], "我们走", POLICY),
            Decision::Strip { seat: 2, chars: 3 }
        );
        // 一样长时取靠前的。
        assert_eq!(
            decide(&["他", "我们", "我们"], "我们走", POLICY),
            Decision::Strip { seat: 1, chars: 2 }
        );
    }

    #[test]
    fn candidates_that_are_not_a_prefix_of_the_gold_are_never_taken() {
        assert_eq!(
            decide(&["", "们", "我们来"], "我们走", POLICY),
            Decision::NotFound
        );
    }

    #[test]
    fn the_panel_is_opened_only_past_the_strip_and_only_for_its_pages() {
        let mut list: Vec<String> = (0..60).map(|index| format!("错{index}")).collect();
        list[30] = "对".into();
        let texts: Vec<&str> = list.iter().map(String::as_str).collect();
        assert_eq!(
            decide(&texts, "对", POLICY),
            Decision::Panel {
                seat: 30,
                chars: 1,
                page: 1
            }
        );
        // 只翻一页时找不到。
        assert_eq!(
            decide(
                &texts,
                "对",
                Policy {
                    panel_pages: 1,
                    ..POLICY
                }
            ),
            Decision::NotFound
        );
        // 不开面板时只看候选栏。
        assert_eq!(
            decide(
                &texts,
                "对",
                Policy {
                    panel_pages: 0,
                    ..POLICY
                }
            ),
            Decision::NotFound
        );
        list[3] = "对".into();
        let texts: Vec<&str> = list.iter().map(String::as_str).collect();
        assert_eq!(
            decide(&texts, "对", POLICY),
            Decision::Strip { seat: 3, chars: 1 }
        );
    }

    #[test]
    fn a_slip_takes_the_first_wrong_candidate_after_the_top() {
        assert_eq!(slip(&["我们", "我", "握们"], "我们", POLICY), Some(2));
        assert_eq!(slip(&["我们"], "我们", POLICY), None);
    }
}
