import {
  CandidateCanvasLease,
  CandidateCanvasLeaseCache,
} from "../entry/src/main/ets/keyboard/candidate/CandidateCanvasLeaseCache";

let assertions = 0;
function check(value: boolean, message: string): void {
  assertions++;
  if (!value) throw new Error(message);
}

class Lease implements CandidateCanvasLease {
  readers: number = 0;
  constructor(readonly key: string) {}
}

let created: string[] = [];
let freed: string[] = [];
const cache = new CandidateCanvasLeaseCache<Lease>((lease: Lease): void => {
  freed.push(lease.key);
});
function retain(key: string): Lease {
  return cache.retain(key, (): Lease => {
    created.push(key);
    return new Lease(key);
  });
}

// 候选条在词与词之间放手又取回，同一套皮肤只解码一次。
const first = retain("1:skin:light");
cache.release("1:skin:light", first);
check(freed.length === 0, "最后一个读者离开后停放，不立即释放");
const again = retain("1:skin:light");
check(again === first && created.length === 1, "同 key 再取用接回停放的解码结果");
check(again.readers === 1, "接回后读者数从零开始累计");

// 候选条与展开区同时读同一份。
const expanded = retain("1:skin:light");
check(expanded === first && first.readers === 2, "同时在用的读者共享一份");
cache.release("1:skin:light", expanded);
check(freed.length === 0 && first.readers === 1, "还有读者时不停放也不释放");
cache.release("1:skin:light", again);

// 换皮肤或切深浅色时 key 变了，旧的停放份被挤掉，不与新皮肤同时驻留。
const dark = retain("2:skin:dark");
check(created.join(",") === "1:skin:light,2:skin:dark", "新 key 重新解码");
check(freed.join(",") === "1:skin:light", "新 key 挤掉停放的旧位图");
cache.release("2:skin:dark", dark);
check(freed.length === 1, "新的那份停放");

// 停放只留一份：另一份离开时替换并释放先停放的那份。
const toolbar = retain("toolbar:3");
check(freed.join(",") === "1:skin:light,2:skin:dark", "取用别的 key 时释放停放份");
cache.release("toolbar:3", toolbar);
check(freed.length === 2, "最近离开的一份停放");

// 键盘收起：释放停放的那份；收起后才离开的读者直接释放，不再停放。
cache.trim();
check(freed.join(",") === "1:skin:light,2:skin:dark,toolbar:3", "收起时释放停放的位图");
cache.trim();
check(freed.length === 3, "重复 trim 不重复释放");
const live = retain("4:skin:light");
cache.trim();
check(freed.length === 3 && live.readers === 1, "trim 不动还有读者的解码结果");
cache.release("4:skin:light", live);
check(
  freed.join(",") === "1:skin:light,2:skin:dark,toolbar:3,4:skin:light",
  "trim 之后离开的读者直接释放",
);
const reopened = retain("4:skin:light");
check(reopened !== live && created.length === 5, "已释放的不会被接回");

// 再次取用后恢复停放。
cache.release("4:skin:light", reopened);
check(freed.length === 4, "取用之后恢复停放");
check(retain("4:skin:light") === reopened && created.length === 5, "恢复停放后再接回");

console.log(`candidate-canvas-lease: ${assertions} assertions passed`);
