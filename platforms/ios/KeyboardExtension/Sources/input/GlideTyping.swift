import CoreGraphics
import Foundation

/// 滑行输入（#5347）里与 UIKit 无关的那部分：一次触摸什么时候算滑行，以及抬手时交给 `msime_client_glide` 的请求怎么拼。
///
/// 所有坐标都在同一个键区坐标系里（键盘用 `KeyAreaStackView` 的坐标），`frames` 是 a..z 二十六个字母键按字母顺序排列的布局矩形。判断是不是滑行完全由宿主决定，Engine 只负责把这一笔解码成全拼字母；Android 和 HarmonyOS 用的是同一套规则。
enum GlideTyping {
  /// 一个触摸采样：键区坐标里的位置，以及从按下那一刻起经过的毫秒数。Engine 用时间认出手指在某个键上的停留。
  struct Sample: Equatable {
    var x: CGFloat
    var y: CGFloat
    var milliseconds: Double
  }

  static let letters = Array("abcdefghijklmnopqrstuvwxyz")
  /// `msime_client_glide` 一次最多接受的采样点数。
  static let pointLimit = 1_024
  /// 手指尚未跨到另一枚键时也限制内存；抬手时还会再抽稀到 `pointLimit`。
  static let sampleBufferLimit = 4_096
  /// `msime_client_glide` 接受的请求最大字节数。
  static let requestByteLimit = 65_536
  /// 横向位移至少达到按下那个键键宽的这个比例才算滑行：同一个键上的竖直手势和手指的轻微抖动都到不了这个距离。
  static let horizontalThreshold: CGFloat = 0.4

  /// `point` 落在哪个字母键的布局矩形里；落在键距里或键区外时为 nil。空矩形（隐藏的键）不参与。
  static func keyIndex(at point: CGPoint, in frames: [CGRect]) -> Int? {
    frames.firstIndex { !$0.isEmpty && $0.contains(point) }
  }

  /// 在 `downKey` 上按下、从 `down` 移到 `current` 的一根手指是否开始滑行：手指此刻在另一个字母键上，而且横向离开按下点至少 `horizontalThreshold` 个键宽。
  static func startsGlide(downKey: Int, down: CGPoint, current: CGPoint, frames: [CGRect]) -> Bool {
    guard frames.indices.contains(downKey), !frames[downKey].isEmpty,
          let key = keyIndex(at: current, in: frames), key != downKey else { return false }
    return abs(current.x - down.x) >= frames[downKey].width * horizontalThreshold
  }

  /// 均匀抽取至多 `limit` 个采样，第一个和最后一个总是保留；不超过上限时原样返回。
  static func downsampled<Element>(_ samples: [Element], limit: Int = pointLimit) -> [Element] {
    guard limit >= 2, samples.count > limit else { return samples }
    let last = samples.count - 1
    return (0..<limit).map { samples[$0 * last / (limit - 1)] }
  }

  /// 追加一个触摸采样，同时限制尚未结束的手势占用的数组大小。
  static func appendBounded(_ sample: Sample, to samples: inout [Sample]) {
    if samples.count >= sampleBufferLimit {
      samples = downsampled(samples, limit: sampleBufferLimit / 2)
    }
    samples.append(sample)
  }

  /// `msime_client_glide` 的请求 JSON：`keys` 是 a..z 各键的中心，`key_width`/`key_height` 是一个字母键的大小（取二十六个键里最小的宽和高），`points` 是至多 `pointLimit` 个 `[x, y, ms]`。坐标保留一位小数、时间取整毫秒，请求因此远小于 `requestByteLimit`。键不全、采样不足两个或有非有限值时为 nil。
  static func request(frames: [CGRect], samples: [Sample]) -> Data? {
    guard frames.count == letters.count, samples.count >= 2,
          frames.allSatisfy({ !$0.isEmpty && $0.minX.isFinite && $0.minY.isFinite && $0.width.isFinite && $0.height.isFinite }),
          samples.allSatisfy({ $0.x.isFinite && $0.y.isFinite && $0.milliseconds.isFinite }),
          let width = frames.map(\.width).min(), let height = frames.map(\.height).min(),
          width > 0, height > 0 else { return nil }
    let keys = frames.map { "[\(number($0.midX)),\(number($0.midY))]" }.joined(separator: ",")
    let points = downsampled(samples).map { sample in
      "[\(number(sample.x)),\(number(sample.y)),\(milliseconds(sample.milliseconds))]"
    }.joined(separator: ",")
    let json = "{\"keys\":[\(keys)],\"key_width\":\(number(width)),\"key_height\":\(number(height)),\"points\":[\(points)]}"
    let data = Data(json.utf8)
    return data.count <= requestByteLimit ? data : nil
  }

  private static func number(_ value: CGFloat) -> String {
    String(format: "%.1f", Double(value))
  }

  private static func milliseconds(_ value: Double) -> UInt32 {
    UInt32(min(max(value, 0), Double(UInt32.max)).rounded())
  }
}
