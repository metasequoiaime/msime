import {
  EMAIL_SUFFIXES,
  EmailSuffixPolicy,
} from "../entry/src/main/ets/keyboard/input/EmailSuffixPolicy";
import { SymbolPanelPolicy } from "../entry/src/main/ets/keyboard/input/SymbolPanelPolicy";

function check(condition: boolean, message: string): void {
  if (!condition) throw new Error(message);
}

const expected = [
  "@qq.com",
  "@163.com",
  "@126.com",
  "@foxmail.com",
  "@139.com",
  "@189.cn",
  "@aliyun.com",
  "@gmail.com",
  "@icloud.com",
  "@yahoo.com",
  "@proton.me",
  "@protonmail.com",
  "@aol.com",
  "@mail.com",
  "@gmx.com",
  "@naver.com",
  "@yahoo.co.jp",
];
const offered = EmailSuffixPolicy.match("review@");
check(EMAIL_SUFFIXES.join(",") === expected.join(","), "出厂后缀目录保留批准的 17 项及顺序");
check(
  offered !== null && offered.suffixes.join(",") === expected.join(","),
  "普通输入框提供完整后缀目录",
);

const network = SymbolPanelPolicy.categories().find((category) => category.id === "network");
check(network !== undefined, "符号面板保留网络分类");
for (const suffix of expected) {
  check(network!.symbols.includes(suffix), `符号面板保留 ${suffix}`);
  const replacement = EmailSuffixPolicy.replacement(offered, suffix);
  check(
    replacement !== null && replacement.deleteCount === 0 && replacement.insert === suffix.slice(1),
    `选择 ${suffix} 只补全域名`,
  );
}

for (const domain of ["outlook.com", "hotmail.com", "live.com", "msn.com"]) {
  const suffix = "@" + domain;
  check(!network!.symbols.includes(suffix), `符号面板不再提供 ${suffix}`);
  check(
    EmailSuffixPolicy.match("review@" + domain.split(".")[0]) === null,
    `输入 ${domain} 的前缀不给出已移除的建议`,
  );
  check(
    EmailSuffixPolicy.replacement(offered, suffix) === null,
    `旧候选 ${suffix} 不能绕过当前目录直接补全`,
  );
}

console.log("邮箱后缀候选、补全与符号面板按 17 项正式目录验收通过");
