//! Win7 的 Tablet PC COM 手写后端；所有接口只在专用 STA 线程内使用。

use super::{chinese_rank, contains_cjk, InkError, Stroke, MAX_POINTS, MAX_STROKES};
use std::mem::ManuallyDrop;
use windows::Win32::System::Com::{
    CoCreateInstance, CoInitializeEx, CoUninitialize, CLSCTX_INPROC_SERVER,
    COINIT_APARTMENTTHREADED, SAFEARRAY,
};
use windows::Win32::System::Ole::{
    SafeArrayAccessData, SafeArrayCreateVector, SafeArrayUnaccessData,
};
use windows::Win32::System::Variant::{
    VariantClear, VARIANT, VARIANT_0, VARIANT_0_0, VARIANT_0_0_0, VT_ARRAY, VT_I4,
};
use windows::Win32::UI::TabletPC::{
    IInkDisp, IInkRecognizer, IInkRecognizers, IRS_NoError, InkDisp, InkRecognizers,
};

const MAX_ALTERNATES: i32 = 32;
const MAX_CANDIDATES: usize = 12;
// 面板坐标为 DIP；COM 的 ink space 使用百分之一毫米。
const HIMETRIC_PER_DIP: f64 = 2540.0 / 96.0;
// 正负极值之间的跨度仍小于 LONG_MAX，满足 CreateStroke 的边界约束。
const COORDINATE_LIMIT: f64 = (i32::MAX / 2) as f64;

struct Apartment;
impl Drop for Apartment {
    fn drop(&mut self) {
        // 所有 COM 对象先于此 guard 析构，初始化与退出始终发生在同一线程。
        unsafe { CoUninitialize() };
    }
}

fn in_sta<T: Send + 'static>(
    operation: impl FnOnce() -> Result<T, InkError> + Send + 'static,
) -> Result<T, InkError> {
    std::thread::Builder::new()
        .name("msime-win7-ink".into())
        .spawn(move || {
            unsafe { CoInitializeEx(None, COINIT_APARTMENTTHREADED) }
                .ok()
                .map_err(|_| InkError::Unavailable)?;
            let _apartment = Apartment;
            operation()
        })
        .map_err(|_| InkError::Unavailable)?
        .join()
        .map_err(|_| InkError::Unavailable)?
}

fn best_recognizer() -> Result<Option<IInkRecognizer>, InkError> {
    let recognizers: IInkRecognizers =
        unsafe { CoCreateInstance(&InkRecognizers, None, CLSCTX_INPROC_SERVER) }
            .map_err(|_| InkError::Unavailable)?;
    let count = unsafe { recognizers.Count() }.map_err(|_| InkError::Unavailable)?;
    let mut best = None;
    let mut best_rank = 0;
    for index in 0..count {
        let recognizer = unsafe { recognizers.Item(index) }.map_err(|_| InkError::Unavailable)?;
        let name = unsafe { recognizer.Name() }.map_err(|_| InkError::Unavailable)?;
        let rank = chinese_rank(&name.to_string());
        if rank > best_rank {
            best_rank = rank;
            best = Some(recognizer);
            if rank == 3 {
                break;
            }
        }
    }
    Ok(best)
}

pub(super) fn has_chinese_recognizer() -> bool {
    in_sta(|| best_recognizer().map(|recognizer| recognizer.is_some())).unwrap_or(false)
}

fn coordinates(stroke: &Stroke) -> Result<Vec<i32>, InkError> {
    let mut result = Vec::with_capacity(stroke.len().min(MAX_POINTS) * 2);
    for &(x, y) in stroke.iter().take(MAX_POINTS) {
        if !x.is_finite() || !y.is_finite() {
            continue;
        }
        for value in [x, y] {
            let converted = (f64::from(value) * HIMETRIC_PER_DIP).round();
            if converted.abs() > COORDINATE_LIMIT {
                return Err(InkError::Unavailable);
            }
            result.push(converted as i32);
        }
    }
    Ok(result)
}

struct PacketData(VARIANT);
impl Drop for PacketData {
    fn drop(&mut self) {
        // Variant 独占 SAFEARRAY；CreateStroke 同步复制数据，不接管数组所有权。
        let _ = unsafe { VariantClear(&mut self.0) };
    }
}

struct ArrayLock(*mut SAFEARRAY);
impl Drop for ArrayLock {
    fn drop(&mut self) {
        let _ = unsafe { SafeArrayUnaccessData(self.0) };
    }
}

impl PacketData {
    fn new(points: &[i32]) -> Result<Self, InkError> {
        let length = u32::try_from(points.len()).map_err(|_| InkError::Unavailable)?;
        if length == 0 || points.len() > MAX_POINTS * 2 {
            return Err(InkError::EmptyInput);
        }
        let array = unsafe { SafeArrayCreateVector(VT_I4, 0, length) };
        if array.is_null() {
            return Err(InkError::Unavailable);
        }
        let value = Self(VARIANT {
            Anonymous: VARIANT_0 {
                Anonymous: ManuallyDrop::new(VARIANT_0_0 {
                    vt: VT_ARRAY | VT_I4,
                    Anonymous: VARIANT_0_0_0 { parray: array },
                    ..Default::default()
                }),
            },
        });
        let mut data = std::ptr::null_mut();
        unsafe { SafeArrayAccessData(array, &mut data) }.map_err(|_| InkError::Unavailable)?;
        let _lock = ArrayLock(array);
        if data.is_null() {
            return Err(InkError::Unavailable);
        }
        // VT_I4 数组的元素正是 i32；锁定期间长度由刚创建的 SAFEARRAY 固定。
        unsafe { std::ptr::copy_nonoverlapping(points.as_ptr(), data.cast::<i32>(), points.len()) };
        Ok(value)
    }
}

fn build_ink(strokes: &[Stroke]) -> Result<IInkDisp, InkError> {
    let ink: IInkDisp = unsafe { CoCreateInstance(&InkDisp, None, CLSCTX_INPROC_SERVER) }
        .map_err(|_| InkError::Unavailable)?;
    let mut count = 0;
    for stroke in strokes.iter().take(MAX_STROKES) {
        let points = coordinates(stroke)?;
        if points.len() < 4 {
            continue;
        }
        let packets = PacketData::new(&points)?;
        unsafe { ink.CreateStroke(&packets.0, &VARIANT::default()) }
            .map_err(|_| InkError::Unavailable)?;
        count += 1;
    }
    if count == 0 {
        return Err(InkError::EmptyInput);
    }
    Ok(ink)
}

fn candidates(values: impl IntoIterator<Item = String>) -> Vec<String> {
    let mut chinese = Vec::new();
    let mut other = Vec::new();
    for value in values {
        if msime_client_core::panels::validate_candidate(&value).is_err() {
            continue;
        }
        let bucket = if contains_cjk(&value) {
            &mut chinese
        } else {
            &mut other
        };
        if !bucket.contains(&value) {
            bucket.push(value);
        }
    }
    chinese.extend(other);
    chinese.truncate(MAX_CANDIDATES);
    chinese
}

pub(super) fn recognize(strokes: &[Stroke]) -> Result<Vec<String>, InkError> {
    // 只复制已有的输入上限，工作线程不持有调用方数据，也不会复制无限长的笔划。
    let strokes: Vec<Stroke> = strokes
        .iter()
        .take(MAX_STROKES)
        .map(|stroke| stroke.iter().take(MAX_POINTS).copied().collect())
        .collect();
    in_sta(move || {
        let ink = build_ink(&strokes)?;
        let recognizer = best_recognizer()?.ok_or(InkError::NoChineseRecognizer)?;
        let context =
            unsafe { recognizer.CreateRecognizerContext() }.map_err(|_| InkError::Unavailable)?;
        let collected = unsafe { ink.Strokes() }.map_err(|_| InkError::Unavailable)?;
        unsafe { context.putref_Strokes(&collected) }.map_err(|_| InkError::Unavailable)?;
        let mut status = IRS_NoError;
        let mut result = None;
        unsafe { context.Recognize(&mut status, &mut result) }
            .map_err(|_| InkError::Unavailable)?;
        if status != IRS_NoError {
            return Err(InkError::Unavailable);
        }
        let Some(result) = result else {
            return Ok(Vec::new());
        };
        let top = unsafe { result.TopString() }.map_err(|_| InkError::Unavailable)?;
        let mut values = vec![top.to_string()];
        if !top.is_empty() {
            let length = i32::try_from(top.len()).map_err(|_| InkError::Unavailable)?;
            let alternates = unsafe { result.AlternatesFromSelection(0, length, MAX_ALTERNATES) }
                .map_err(|_| InkError::Unavailable)?;
            let count = unsafe { alternates.Count() }.map_err(|_| InkError::Unavailable)?;
            for index in 0..count.min(MAX_ALTERNATES) {
                let alternate =
                    unsafe { alternates.Item(index) }.map_err(|_| InkError::Unavailable)?;
                let value = unsafe { alternate.String() }.map_err(|_| InkError::Unavailable)?;
                values.push(value.to_string());
            }
        }
        Ok(candidates(values))
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn packet_conversion_preserves_geometry_and_bounds_invalid_coordinates() {
        assert_eq!(
            coordinates(&vec![(0.0, 0.0), (96.0, 192.0)]),
            Ok(vec![0, 0, 2540, 5080])
        );
        assert_eq!(
            coordinates(&vec![(f32::MAX, 0.0)]),
            Err(InkError::Unavailable)
        );
        assert!(coordinates(&vec![(f32::NAN, 1.0), (0.0, f32::INFINITY)])
            .unwrap()
            .is_empty());
        assert_eq!(
            coordinates(&vec![(1.0, 1.0); MAX_POINTS + 1])
                .unwrap()
                .len(),
            MAX_POINTS * 2
        );
    }

    #[test]
    fn native_ink_copies_bounded_synthetic_packets() {
        in_sta(|| {
            let ink = build_ink(&[vec![(0.0, 0.0), (96.0, 192.0)]])?;
            let strokes = unsafe { ink.Strokes() }.map_err(|_| InkError::Unavailable)?;
            assert_eq!(unsafe { strokes.Count() }.unwrap(), 1);
            let stroke = unsafe { strokes.Item(0) }.unwrap();
            assert_eq!(unsafe { stroke.PacketCount() }.unwrap(), 2);
            // 原输入数组已经释放；从 COM 笔划读回实际坐标，验证其拥有独立副本。
            let points = PacketData(unsafe { stroke.GetPoints(0, 2) }.unwrap());
            let variant = unsafe { &points.0.Anonymous.Anonymous };
            assert_eq!(variant.vt, VT_ARRAY | VT_I4);
            let array = unsafe { variant.Anonymous.parray };
            assert_eq!(
                unsafe { windows::Win32::System::Ole::SafeArrayGetLBound(array, 1) }.unwrap(),
                0
            );
            assert_eq!(
                unsafe { windows::Win32::System::Ole::SafeArrayGetUBound(array, 1) }.unwrap(),
                3
            );
            let mut data = std::ptr::null_mut();
            unsafe { SafeArrayAccessData(array, &mut data) }.unwrap();
            let locked = ArrayLock(array);
            assert!(!data.is_null());
            assert_eq!(
                unsafe { std::slice::from_raw_parts(data.cast::<i32>(), 4) },
                &[0, 0, 2540, 5080]
            );
            drop(locked);
            assert!(matches!(
                build_ink(&[vec![(1.0, 1.0)]]),
                Err(InkError::EmptyInput)
            ));
            assert!(matches!(
                build_ink(&[vec![(f32::NAN, 0.0); 2]]),
                Err(InkError::EmptyInput)
            ));
            Ok(())
        })
        .unwrap();
    }

    #[test]
    fn candidates_keep_chinese_first_deduplicate_and_obey_panel_contract() {
        let result = candidates([
            "a".into(),
            "十".into(),
            "十".into(),
            "".into(),
            "bad\nvalue".into(),
            "x".repeat(257),
        ]);
        assert_eq!(result, vec!["十", "a"]);
        let many = candidates((0..40).map(|index| format!("合成{index}")));
        assert_eq!(many.len(), MAX_CANDIDATES);
    }

    #[test]
    fn installed_recognizer_performs_synthetic_cross_recognition_when_available() {
        let installed =
            in_sta(|| best_recognizer().map(|recognizer| recognizer.is_some())).unwrap();
        assert_eq!(has_chinese_recognizer(), installed);
        if !installed {
            eprintln!(
                "Windows 7 COM: no Chinese recognizer installed; native packet tests still ran"
            );
            return;
        }
        let strokes = [
            vec![(30.0, 60.0), (90.0, 60.0), (150.0, 60.0)],
            vec![(90.0, 20.0), (90.0, 60.0), (90.0, 120.0)],
        ];
        let result = recognize(&strokes).unwrap();
        assert!(!result.is_empty());
        assert!(result.iter().any(|text| text == "十"));
        assert!(result.len() <= MAX_CANDIDATES);
        eprintln!("Windows 7 COM: real Chinese recognizer accepted the synthetic cross");
        // 通过公共入口覆盖目标选择，普通 Windows 在此实际执行原有 WinRT 后端。
        let public_result = super::super::recognize(&strokes).unwrap();
        assert!(public_result.iter().any(|text| text == "十"));
    }
}
