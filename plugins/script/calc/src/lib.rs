//! calc：`/` 指令扩展参数（`arguments = "extended"`）的参考样板。
//!
//! 只声明 `command` 钩子，只申请 `provide_commands`，清单声明一条指令 `/calc`，写了 `arguments = "extended"`：打完触发词之后，输入法把数字和 `. + - * / % ( ) = ,` 也当作参数交给插件（字符集以 `abi::EXTENDED_ARGUMENT_SYMBOLS` 为准）。插件把参数当算式计算，给出最多三行：
//!
//! | 行 | 例子（`/calc12*(3+4)`） | 注释 |
//! | --- | --- | --- |
//! | 结果 | `84` | 结果（除不尽时是「约等于」） |
//! | 算式=结果 | `12*(3+4)=84`（除不尽时用 `≈`） | 算式 |
//! | 千分位 | 整数部分有 5 位以上时，例如 `12,345,678` | 千分位 |
//!
//! 规则：
//! - 运算符 `+ - * / %`，先乘除取余后加减，同级从左到右；`%` 是取余（结果的符号跟被除数，和输入法计算模式 `V` 相同）；括号可以嵌套；数前面可以有正负号（`-3`、`2*-3`）。
//! - 数可以是整数或小数（`12`、`0.5`、`.5`、`5.`），整数部分可以用 `,` 按三位分组（`1,000,000`），分组不对（`1,00`）算写错。
//! - 末尾的 `=` 去掉不算（`1+2=`），别处出现 `=` 算写错。
//! - 按分数精确计算（分子分母都是 64 位整数），所以 `0.1+0.2` 是 `0.3`、`1/3*3` 是 `1`。小数部分不超过 10 位时原样写出，否则四舍五入保留 10 位小数、去掉末尾的 0，注释写「约等于」。
//! - 不给任何行的情况：参数为空（`/calc`）、算式没写完或写错（`1+`、`(1+2`、`1,00`）、除以 0 或对 0 取余、中间结果超出 64 位整数。输入法这时只显示原样的那一行，日志写明原因（`skip: empty`、`skip: syntax`、`skip: divide by zero`、`skip: overflow`）。
//! - 只有一个数、没有运算符时（`/calc1,000`）只给结果行和千分位行，不给「算式=结果」。
//!
//! 工作量有上限：参数最多 64 字节，每个字节最多进出栈各一次，每次运算一次约分。最坏情况的 fuel 见 README。

#![no_std]

use core::ptr::addr_of_mut;
use msime_script_sdk::{command, hook, log, Error};

/// 参数最多 64 字节（宿主的上限），栈也按这个开。
const MAX_ARGUMENT: usize = 64;
/// 除不尽时保留的小数位数。
const FRACTION_DIGITS: usize = 10;
/// 结果最长：负号、19 位整数、小数点、10 位小数。
const RESULT_BYTES: usize = 32;
/// 千分位写法最长：再加 6 个逗号。
const GROUPED_BYTES: usize = RESULT_BYTES + 8;
/// 行文本的上限（UTF-16 码元）。
const ROW_UNITS: usize = 64;

/// 运算符栈里的一元负号和左括号。二元运算符就是它们的 ASCII 字节。
const NEG: u8 = b'n';
const OPEN: u8 = b'(';

#[no_mangle]
pub extern "C" fn msime_abi() -> i32 {
    msime_script_sdk::ABI_VERSION
}

#[no_mangle]
pub extern "C" fn msime_on_event(kind: i32) -> i32 {
    if kind == hook::COMMAND {
        on_command();
    }
    0
}

/// 算不出结果的原因，写进日志。
#[derive(Clone, Copy)]
enum Fail {
    Empty,
    Syntax,
    DivideByZero,
    Overflow,
}

type Calc<T> = Result<T, Fail>;

fn on_command() {
    let mut argument = [0u8; MAX_ARGUMENT];
    let expression = match command::text(command::ARGUMENT, &mut argument) {
        Ok(text) => trim_equals(text.as_bytes()),
        Err(error) => return log::debug(reason(error)),
    };
    let (value, operators) = match evaluate(expression) {
        Ok(result) => result,
        Err(fail) => return log::debug(failure(fail)),
    };
    let mut result = Text::<RESULT_BYTES>::new();
    let exact = value.write(&mut result);

    let mut rows = 0;
    if !offer(
        result.text(),
        if exact { "结果" } else { "约等于" },
        &mut rows,
    ) {
        return;
    }
    // 「算式=结果」超过一行的上限时不给这一行，日志里注明。
    let mut too_long = false;
    if operators > 0 {
        // 算式只含 ASCII，`≈` 是一个 UTF-16 码元：码元数就是字节数减去 `≈` 多出的 2。
        let mut equation = Text::<{ MAX_ARGUMENT + 3 + RESULT_BYTES }>::new();
        equation.push(expression);
        equation.push(if exact { "=" } else { "≈" }.as_bytes());
        equation.push(result.bytes());
        let units = equation.len - if exact { 0 } else { 2 };
        if units > ROW_UNITS {
            too_long = true;
        } else if !offer(equation.text(), "算式", &mut rows) {
            return;
        }
    }
    if value.whole_digits() >= 5 {
        let mut grouped = Text::<GROUPED_BYTES>::new();
        group(result.bytes(), &mut grouped);
        if !offer(grouped.text(), "千分位", &mut rows) {
            return;
        }
    }
    log::debug(match (rows, too_long) {
        (1, false) => "calc: 1 row",
        (1, true) => "calc: 1 row, equation too long",
        (2, false) => "calc: 2 rows",
        (2, true) => "calc: 2 rows, equation too long",
        _ => "calc: 3 rows",
    });
}

/// 交出一行。宿主拒绝时写日志并返回 `false`，后面的行不再交。
fn offer(text: &str, note: &str, rows: &mut u32) -> bool {
    match command::row(text, note) {
        Ok(_) => {
            *rows += 1;
            true
        }
        Err(error) => {
            log::debug(reason(error));
            false
        }
    }
}

/// 去掉末尾的 `=`。
fn trim_equals(mut bytes: &[u8]) -> &[u8] {
    while let [rest @ .., b'='] = bytes {
        bytes = rest;
    }
    bytes
}

// ---- 分数 ----

/// 一个有理数：分母为正，分子分母互素。
#[derive(Clone, Copy)]
struct Ratio {
    num: i64,
    den: i64,
}

impl Ratio {
    /// `num / den` 约分并把符号放到分子上。
    #[inline(never)]
    fn new(num: i64, den: i64) -> Calc<Self> {
        if den == 1 {
            return Ok(Self { num, den });
        }
        if den == 0 {
            return Err(Fail::DivideByZero);
        }
        let (num, den) = if den < 0 {
            (negate(num)?, negate(den)?)
        } else {
            (num, den)
        };
        let divisor = gcd(num.unsigned_abs(), den as u64) as i64;
        Ok(Self {
            num: num / divisor,
            den: den / divisor,
        })
    }

    /// 两个整数相加不用通分约分。分数的加法放在不内联的 [`Self::add_fractions`] 里：wasmi 进入函数时按函数体的全部指令扣 fuel，整数算式不该为没走到的分数路径付钱。
    #[inline(never)]
    fn add(self, other: Self) -> Calc<Self> {
        if self.den == 1 && other.den == 1 {
            let num = self.num.checked_add(other.num).ok_or(Fail::Overflow)?;
            return Ok(Self { num, den: 1 });
        }
        self.add_fractions(other)
    }

    #[inline(never)]
    fn add_fractions(self, other: Self) -> Calc<Self> {
        let divisor = gcd(self.den as u64, other.den as u64) as i64;
        let left = mul(self.num, other.den / divisor)?;
        let right = mul(other.num, self.den / divisor)?;
        let num = left.checked_add(right).ok_or(Fail::Overflow)?;
        Self::new(num, mul(self.den, other.den / divisor)?)
    }

    #[inline(never)]
    fn sub(self, other: Self) -> Calc<Self> {
        self.add(other.neg()?)
    }

    #[inline(always)]
    fn neg(self) -> Calc<Self> {
        Ok(Self {
            num: negate(self.num)?,
            den: self.den,
        })
    }

    /// 理由同 [`Self::add`]。
    #[inline(never)]
    fn mul(self, other: Self) -> Calc<Self> {
        if self.den == 1 && other.den == 1 {
            return Ok(Self {
                num: mul(self.num, other.num)?,
                den: 1,
            });
        }
        self.mul_fractions(other)
    }

    #[inline(never)]
    fn mul_fractions(self, other: Self) -> Calc<Self> {
        // 先交叉约分，中间结果不容易溢出。
        let first = gcd(self.num.unsigned_abs(), other.den as u64).max(1) as i64;
        let second = gcd(other.num.unsigned_abs(), self.den as u64).max(1) as i64;
        let num = mul(self.num / first, other.num / second)?;
        let den = mul(self.den / second, other.den / first)?;
        Self::new(num, den)
    }

    #[inline(never)]
    fn div(self, other: Self) -> Calc<Self> {
        if other.num == 0 {
            return Err(Fail::DivideByZero);
        }
        // 两个整数：商就是这个分数本身。
        if self.den == 1 && other.den == 1 {
            return Self::new(self.num, other.num);
        }
        self.mul(Self::new(other.den, other.num)?)
    }

    /// 取余：`self - other * trunc(self / other)`，结果的符号跟被除数。
    #[inline(never)]
    fn rem(self, other: Self) -> Calc<Self> {
        if other.num == 0 {
            return Err(Fail::DivideByZero);
        }
        // 两个整数：Rust 的 `%` 就是截断取余，符号跟被除数；`checked_rem` 只为 `i64::MIN % -1` 这一种溢出。
        if self.den == 1 && other.den == 1 {
            let num = self.num.checked_rem(other.num).ok_or(Fail::Overflow)?;
            return Ok(Self { num, den: 1 });
        }
        // 除数不为 0；`checked_div` 只为 `i64::MIN / -1` 这一种溢出。
        let quotient = mul(self.num, other.den)?
            .checked_div(mul(self.den, other.num)?)
            .ok_or(Fail::Overflow)?;
        self.add(other.mul(Self::new(quotient, 1)?)?.neg()?)
    }

    /// 整数部分有几位（不算负号）。
    fn whole_digits(self) -> u32 {
        let mut whole = self.num.unsigned_abs() / self.den as u64;
        let mut digits = 1;
        while whole >= 10 {
            whole /= 10;
            digits += 1;
        }
        digits
    }

    /// 写成十进制，返回是否精确：有限小数原样写出，否则四舍五入到 [`FRACTION_DIGITS`] 位并去掉末尾的 0。
    fn write(self, out: &mut Text<RESULT_BYTES>) -> bool {
        let den = self.den as u64;
        let mut whole = self.num.unsigned_abs() / den;
        let mut rest = self.num.unsigned_abs() % den;
        let mut fraction = [0u8; FRACTION_DIGITS];
        // 余数为 0 就是写完了，整数不进这个循环。
        let mut digits = 0;
        while digits < FRACTION_DIGITS && rest != 0 {
            (fraction[digits], rest) = next_digit(rest, den);
            digits += 1;
        }
        let exact = rest == 0;
        if !exact && next_digit(rest, den).0 >= 5 && carry(&mut fraction) {
            whole += 1;
        }
        let mut length = FRACTION_DIGITS;
        while length > 0 && fraction[length - 1] == 0 {
            length -= 1;
        }
        if self.num < 0 && (whole > 0 || length > 0) {
            out.push(b"-");
        }
        out.number(whole);
        if length > 0 {
            out.push(b".");
            for digit in &fraction[..length] {
                out.push(&[b'0' + digit]);
            }
        }
        exact
    }
}

/// 长除法的下一位：`rest * 10` 除以 `den` 的商和余数，`rest < den`。
#[inline(never)]
fn next_digit(rest: u64, den: u64) -> (u8, u64) {
    if rest <= u64::MAX / 10 {
        let wide = rest * 10;
        return ((wide / den) as u8, wide % den);
    }
    next_digit_wide(rest, den)
}

/// `rest * 10` 超出 64 位时按 128 位算。128 位除法是编译器内置库里的一个长函数，只在分母极大时才走到。
#[inline(never)]
fn next_digit_wide(rest: u64, den: u64) -> (u8, u64) {
    let wide = u128::from(rest) * 10;
    let den = u128::from(den);
    ((wide / den) as u8, (wide % den) as u64)
}

/// 小数部分末位加 1 并向前进位，进到整数部分时返回 `true`。
fn carry(fraction: &mut [u8; FRACTION_DIGITS]) -> bool {
    for digit in fraction.iter_mut().rev() {
        if *digit < 9 {
            *digit += 1;
            return false;
        }
        *digit = 0;
    }
    true
}

#[inline(never)]
fn gcd(mut a: u64, mut b: u64) -> u64 {
    while b != 0 {
        (a, b) = (b, a % b);
    }
    a
}

/// 两个都在 32 位以内时乘积不会溢出 64 位，直接乘；否则走带溢出检查的乘法。wasm 里 64 位乘法的溢出检查要用 128 位乘法，是一个长函数，放在不内联的 [`mul_checked`] 里，常见的小数字不付它的 fuel。
#[inline(always)]
fn mul(a: i64, b: i64) -> Calc<i64> {
    if a.unsigned_abs() <= u64::from(u32::MAX >> 1) && b.unsigned_abs() <= u64::from(u32::MAX >> 1)
    {
        return Ok(a * b);
    }
    mul_checked(a, b)
}

#[inline(never)]
fn mul_checked(a: i64, b: i64) -> Calc<i64> {
    a.checked_mul(b).ok_or(Fail::Overflow)
}

#[inline(always)]
fn negate(value: i64) -> Calc<i64> {
    value.checked_neg().ok_or(Fail::Overflow)
}

// ---- 求值：调度场算法，不递归 ----

/// 运算符栈和数栈。参数最多 64 字节，每个字节最多压一个运算符或一个数，栈不会满。放在静态变量里：在栈上开要先把 1 KiB 清零，每次求值都付这笔 fuel；静态变量全是 0，落在零初始化段里，也不占模块的字节。
static mut MACHINE: Machine = Machine {
    values: [Ratio { num: 0, den: 0 }; MAX_ARGUMENT],
    value_len: 0,
    ops: [0; MAX_ARGUMENT],
    op_len: 0,
};

struct Machine {
    values: [Ratio; MAX_ARGUMENT],
    value_len: usize,
    ops: [u8; MAX_ARGUMENT],
    op_len: usize,
}

impl Machine {
    #[inline(always)]
    fn push_value(&mut self, value: Ratio) {
        self.values[self.value_len] = value;
        self.value_len += 1;
    }

    #[inline(always)]
    fn pop_value(&mut self) -> Calc<Ratio> {
        if self.value_len == 0 {
            return Err(Fail::Syntax);
        }
        self.value_len -= 1;
        Ok(self.values[self.value_len])
    }

    #[inline(always)]
    fn push_op(&mut self, op: u8) {
        self.ops[self.op_len] = op;
        self.op_len += 1;
    }

    /// 把栈顶优先级不低于 `floor` 的运算符都算掉（遇到左括号停）。二元运算符左结合，所以同级的也算掉。
    #[inline(never)]
    fn reduce(&mut self, floor: u8) -> Calc<()> {
        while self.op_len > 0 {
            let op = self.ops[self.op_len - 1];
            if op == OPEN || precedence(op) < floor {
                break;
            }
            self.op_len -= 1;
            self.apply(op)?;
        }
        Ok(())
    }

    /// 右括号：算到对应的左括号并把它弹掉。没有左括号是写错。
    fn close(&mut self) -> Calc<()> {
        self.reduce(0)?;
        if self.op_len == 0 {
            return Err(Fail::Syntax);
        }
        self.op_len -= 1;
        Ok(())
    }

    /// 算完全部：栈里还剩左括号是没配对。
    fn finish(&mut self) -> Calc<Ratio> {
        self.reduce(0)?;
        if self.op_len > 0 || self.value_len != 1 {
            return Err(Fail::Syntax);
        }
        Ok(self.values[0])
    }

    /// 弹出操作数、算一步、压回结果。二元运算交给不内联的 [`binary`]，这个函数本身保持很小：每一步都要进它一次，wasmi 进函数时按函数体的全部指令扣 fuel。
    #[inline(never)]
    fn apply(&mut self, op: u8) -> Calc<()> {
        let right = self.pop_value()?;
        let value = if op == NEG {
            right.neg()?
        } else {
            binary(op, self.pop_value()?, right)?
        };
        self.push_value(value);
        Ok(())
    }
}

/// 二元运算：每种运算各是一个不内联的函数，这里只分派。
#[inline(never)]
fn binary(op: u8, left: Ratio, right: Ratio) -> Calc<Ratio> {
    match op {
        b'+' => left.add(right),
        b'-' => left.sub(right),
        b'*' => left.mul(right),
        b'/' => left.div(right),
        _ => left.rem(right),
    }
}

#[inline(always)]
fn precedence(op: u8) -> u8 {
    match op {
        b'+' | b'-' => 1,
        b'*' | b'/' | b'%' => 2,
        NEG => 3,
        _ => 0,
    }
}

/// 计算算式，返回结果和二元运算符的个数。
///
/// 逐字节的循环体只有一次分派：wasmi 每轮循环按循环体词法上的全部指令扣 fuel，所以读数、压栈、规约都放进各自不内联的函数里，循环本身不随它们变大。
fn evaluate(expression: &[u8]) -> Calc<(Ratio, u32)> {
    if expression.is_empty() {
        return Err(Fail::Empty);
    }
    // SAFETY: 插件单线程运行；每次 `command` 都在 `msime_init` 之后的快照上，这里只在这一次求值里用。
    let machine = unsafe { &mut *addr_of_mut!(MACHINE) };
    machine.value_len = 0;
    machine.op_len = 0;
    let mut operators = 0;
    // 下一个应该是数（或者左括号、正负号），还是运算符（或者右括号）。
    let mut operand = true;
    let mut i = 0;
    while i < expression.len() {
        if operand {
            (i, operand) = machine.operand(expression, i)?;
        } else {
            let binary = machine.operator(expression[i])?;
            operators += u32::from(binary);
            operand = binary;
            i += 1;
        }
    }
    if operand {
        return Err(Fail::Syntax);
    }
    Ok((machine.finish()?, operators))
}

impl Machine {
    /// 该是数的位置上的一个字节：数（读完整个数，之后该是运算符）、左括号、正负号。返回下一个位置和之后是不是还该是数。
    #[inline(never)]
    fn operand(&mut self, expression: &[u8], i: usize) -> Calc<(usize, bool)> {
        match expression[i] {
            b'0'..=b'9' | b'.' => {
                let (value, next) = number(expression, i)?;
                self.push_value(value);
                return Ok((next, false));
            }
            b'(' => self.push_op(OPEN),
            b'-' => self.push_op(NEG),
            b'+' => {}
            _ => return Err(Fail::Syntax),
        }
        Ok((i + 1, true))
    }

    /// 该是运算符的位置上的一个字节：二元运算符（之后该是数，返回 `true`）或右括号（返回 `false`）。
    #[inline(never)]
    fn operator(&mut self, byte: u8) -> Calc<bool> {
        match byte {
            b'+' | b'-' | b'*' | b'/' | b'%' => {
                self.reduce(precedence(byte))?;
                self.push_op(byte);
                Ok(true)
            }
            b')' => {
                self.close()?;
                Ok(false)
            }
            _ => Err(Fail::Syntax),
        }
    }
}

/// 从 `start` 读一个数：整数部分（可以按三位用 `,` 分组）、可选的小数点和小数部分，至少一位数字。返回它的值和数后面的位置。
///
/// 逐位的循环只做「是不是数字、会不会溢出、乘 10 加上它」：带溢出检查的 64 位乘法在 wasm 里要走 128 位乘法，每位一两百 fuel；这里先和 `(i64::MAX - 位) / 10` 比，比 128 位乘法便宜得多。分组的逗号少见，单独在 [`check_groups`] 里核对。
#[inline(never)]
fn number(text: &[u8], start: usize) -> Calc<(Ratio, usize)> {
    let mut num: i64 = 0;
    let mut den: i64 = 1;
    let mut digits = 0;
    let mut i = start;
    while i < text.len() {
        let byte = text[i];
        if byte == b',' {
            i += 1;
            continue;
        }
        let digit = byte.wrapping_sub(b'0');
        if digit > 9 {
            break;
        }
        num = push_digit(num, digit)?;
        digits += 1;
        i += 1;
    }
    let whole_end = i;
    if text[start..whole_end].contains(&b',') {
        check_groups(&text[start..whole_end])?;
    }
    if i < text.len() && text[i] == b'.' {
        i += 1;
        while i < text.len() {
            let digit = text[i].wrapping_sub(b'0');
            if digit > 9 {
                break;
            }
            num = push_digit(num, digit)?;
            // 分母是 10 的幂，不超过分子的位数，分子没溢出它就不会溢出。
            den *= 10;
            digits += 1;
            i += 1;
        }
    }
    if digits == 0 {
        return Err(Fail::Syntax);
    }
    let value = if den == 1 {
        Ratio { num, den }
    } else {
        Ratio::new(num, den)?
    };
    Ok((value, i))
}

/// `num * 10 + digit`，超出 64 位时是 `Overflow`。
#[inline(always)]
fn push_digit(num: i64, digit: u8) -> Calc<i64> {
    let digit = i64::from(digit);
    if num > (i64::MAX - digit) / 10 {
        return Err(Fail::Overflow);
    }
    Ok(num * 10 + digit)
}

/// 整数部分的逗号分组：第一组 1 到 3 位，之后每组恰好 3 位（`1,000,000`）。`1,00`、`1,0000`、`,1`、`1,,000` 都是写错。
#[inline(never)]
fn check_groups(whole: &[u8]) -> Calc<()> {
    let mut group = 0;
    let mut grouped = false;
    for &byte in whole {
        if byte == b',' {
            if group == 0 || group > 3 || (grouped && group != 3) {
                return Err(Fail::Syntax);
            }
            grouped = true;
            group = 0;
        } else {
            group += 1;
        }
    }
    if group != 3 {
        return Err(Fail::Syntax);
    }
    Ok(())
}

// ---- 输出 ----

/// 拼一行文本的固定缓冲，只拼 ASCII 和 `≈`。
struct Text<const N: usize> {
    bytes: [u8; N],
    len: usize,
}

impl<const N: usize> Text<N> {
    fn new() -> Self {
        Self {
            bytes: [0; N],
            len: 0,
        }
    }

    fn push(&mut self, text: &[u8]) {
        self.bytes[self.len..self.len + text.len()].copy_from_slice(text);
        self.len += text.len();
    }

    /// 无符号十进制。
    fn number(&mut self, mut value: u64) {
        let mut digits = [0u8; 20];
        let mut count = 0;
        loop {
            digits[count] = b'0' + (value % 10) as u8;
            count += 1;
            value /= 10;
            if value == 0 {
                break;
            }
        }
        while count > 0 {
            count -= 1;
            self.push(&[digits[count]]);
        }
    }

    fn bytes(&self) -> &[u8] {
        &self.bytes[..self.len]
    }

    /// 拼进去的都是完整的 UTF-8 片段。
    fn text(&self) -> &str {
        core::str::from_utf8(self.bytes()).unwrap_or("")
    }
}

/// 把 `result` 的整数部分每三位加一个逗号，负号和小数部分原样。
fn group(result: &[u8], out: &mut Text<GROUPED_BYTES>) {
    let (sign, rest) = match result {
        [b'-', rest @ ..] => (&b"-"[..], rest),
        _ => (&b""[..], result),
    };
    let whole = rest
        .iter()
        .position(|&byte| byte == b'.')
        .unwrap_or(rest.len());
    out.push(sign);
    for (i, digit) in rest[..whole].iter().enumerate() {
        if i > 0 && (whole - i) % 3 == 0 {
            out.push(b",");
        }
        out.push(&[*digit]);
    }
    out.push(&rest[whole..]);
}

fn failure(fail: Fail) -> &'static str {
    match fail {
        Fail::Empty => "skip: empty",
        Fail::Syntax => "skip: syntax",
        Fail::DivideByZero => "skip: divide by zero",
        Fail::Overflow => "skip: overflow",
    }
}

fn reason(error: Error) -> &'static str {
    match error {
        Error::Denied => "skip: denied",
        Error::Absent => "skip: absent",
        Error::Limit => "skip: limit",
        _ => "skip: invalid",
    }
}

#[panic_handler]
fn panic(_: &core::panic::PanicInfo) -> ! {
    core::arch::wasm32::unreachable()
}
