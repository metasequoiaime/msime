import Foundation

/// The microphone level the record button's waveform draws, the iOS counterpart of the Windows voice overlay's input level. Both recording paths reduce to decibels: the file recorder meters them itself, and a live recording measures each PCM16 buffer it hands out.
enum VoiceLevel {
  /// Bars in the waveform; each holds one sample, newest on the right.
  static let history = 24
  /// Below this, speech is indistinguishable from room noise and the bar stays flat.
  static let floorDecibels: Float = -50

  /// A decibel reading as a bar height from 0 to 1.
  static func normalized(decibels: Float) -> Float {
    guard decibels.isFinite else { return 0 }
    return SharedNumber.clamped((decibels - floorDecibels) / -floorDecibels, to: 0...1)
  }

  /// The RMS level of little-endian PCM16 samples, in decibels relative to full scale; silence and empty buffers read as the floor.
  static func decibels(pcm16 data: Data) -> Float {
    let count = data.count / 2
    guard count > 0 else { return floorDecibels }
    var sum: Double = 0
    data.withUnsafeBytes { raw in
      for index in 0..<count {
        let sample = Double(Int16(littleEndian: raw.loadUnaligned(fromByteOffset: index * 2, as: Int16.self))) / 32_768
        sum += sample * sample
      }
    }
    let rms = (sum / Double(count)).squareRoot()
    guard rms > 0 else { return floorDecibels }
    return max(floorDecibels, Float(20 * log10(rms)))
  }

  /// The waveform after one more sample: the oldest bar drops off the left.
  static func appending(_ level: Float, to levels: [Float]) -> [Float] {
    Array((levels + [level]).suffix(history))
  }
}
