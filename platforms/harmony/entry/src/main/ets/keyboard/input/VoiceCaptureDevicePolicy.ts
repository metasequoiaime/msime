/**
 * Choosing which microphone the recognizers record from.
 *
 * The shared settings store a backend name beside the device, and the backend is what says who the
 * device id belongs to. A Windows endpoint id and a HarmonyOS device address are both strings, and
 * reading one as the other would silently record from the wrong microphone — or from none — on a
 * profile carried over from another machine, so anything that does not name this host is refused
 * rather than reinterpreted. Empty and `auto` mean the user has expressed no preference and the
 * system default stands.
 *
 * The id is composed rather than taken from the descriptor's `id`, which is a handle the audio
 * service hands out per session and does not survive a reboot. Device type and address together do:
 * the built-in microphone keeps an empty address, which is stable precisely because it is always
 * the same device.
 */
const MAX_ID_LENGTH: number = 256;
const MAX_LABEL_LENGTH: number = 128;

export const HARMONY_CAPTURE_BACKEND: string = 'harmony';

export interface VoiceCaptureDevice {
  readonly backend: string;
  readonly id: string;
  readonly label: string;
}

export class VoiceCaptureDevicePolicy {
  /** Whether a stored backend names this host's enumeration at all. */
  static selects(backend: string | null): boolean {
    const value: string = (backend ?? '').trim();
    return value.length === 0 || value === 'auto' || value === HARMONY_CAPTURE_BACKEND;
  }

  /**
   * A device identity that survives a restart.
   *
   * Refuses anything it cannot render as one line: a control character in a device name reaches a
   * preference file, a settings page and a log, and none of those want it.
   */
  static stableId(deviceType: number, address: string | null): string {
    if (!Number.isInteger(deviceType) || deviceType < 0) {
      return '';
    }
    const suffix: string = VoiceCaptureDevicePolicy.printable(address ?? '');
    const id: string = `${deviceType}:${suffix}`;
    return id.length > MAX_ID_LENGTH ? '' : id;
  }

  /** The most specific name the descriptor offers, bounded, with the type as the last resort. */
  static label(displayName: string | null, name: string | null, deviceType: number): string {
    const candidates: string[] = [displayName ?? '', name ?? ''];
    for (const candidate of candidates) {
      const printable: string = VoiceCaptureDevicePolicy.printable(candidate).trim();
      if (printable.length > 0) {
        if (printable.length <= MAX_LABEL_LENGTH) {
          return printable;
        }
        let end: number = MAX_LABEL_LENGTH - 1;
        // 避免截断 emoji 时把孤立的高代理项带入设备名称。
        if (
          end > 0 &&
          printable.charCodeAt(end - 1) >= 0xd800 &&
          printable.charCodeAt(end - 1) <= 0xdbff
        ) {
          end -= 1;
        }
        return printable.substring(0, end) + '…';
      }
    }
    return `录音设备 ${Number.isInteger(deviceType) ? deviceType : 0}`;
  }

  /**
   * The enumerated device the stored choice names, or null for the system default.
   *
   * Null covers three cases that are all "use whatever the system picks": no choice saved, a choice
   * belonging to another host's backend, and a device that is no longer plugged in. The last is the
   * reason this does not fail: a keyboard that refuses to record because a headset was unplugged is
   * worse than one that records from the built-in microphone.
   */
  static match(devices: VoiceCaptureDevice[], backend: string | null,
               deviceId: string | null): VoiceCaptureDevice | null {
    if (!VoiceCaptureDevicePolicy.selects(backend)) {
      return null;
    }
    const wanted: string = (deviceId ?? '').trim();
    if (wanted.length === 0) {
      return null;
    }
    for (const device of devices) {
      if (device.backend === HARMONY_CAPTURE_BACKEND && device.id === wanted) {
        return device;
      }
    }
    return null;
  }

  private static printable(value: string): string {
    let result: string = '';
    for (const character of value) {
      const code: number = character.charCodeAt(0);
      if (code >= 0x20 && code !== 0x7f) {
        result += character;
      }
    }
    return result;
  }
}
