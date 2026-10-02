//! Local message-pipe adapter. One worker owns its handles and drains cancelled
//! overlapped I/O before releasing buffers; no recognition text is logged.
use msime_client_core::voice::controller::{self as protocol, Error, Transport, Update};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};
use windows_sys::Win32::Foundation::*;
use windows_sys::Win32::Security::*;
use windows_sys::Win32::Storage::FileSystem::*;
use windows_sys::Win32::System::Pipes::*;
use windows_sys::Win32::System::RemoteDesktop::ProcessIdToSessionId;
use windows_sys::Win32::System::Threading::*;
use windows_sys::Win32::System::IO::*;

/// Dedicated control endpoint used by the Windows Server.
pub const PIPE_NAME: &str = r"\\.\pipe\FanyImeVoiceControlNamedPipe";
struct Handle(HANDLE);
impl Drop for Handle {
    fn drop(&mut self) {
        // SAFETY: each Handle owns a valid, non-pseudo handle exactly once.
        unsafe {
            CloseHandle(self.0);
        }
    }
}
fn token(process: HANDLE) -> Result<Handle, Error> {
    let mut handle = std::ptr::null_mut();
    // SAFETY: process is retained or the current-process pseudo handle; output is valid.
    if unsafe { OpenProcessToken(process, TOKEN_QUERY, &mut handle) } == 0 {
        return Err(Error::Denied);
    }
    Ok(Handle(handle))
}
fn user(token: &Handle) -> Result<Vec<usize>, Error> {
    let mut length = 0;
    // SAFETY: size query writes only length.
    unsafe {
        GetTokenInformation(token.0, TokenUser, std::ptr::null_mut(), 0, &mut length);
    }
    if length < std::mem::size_of::<TOKEN_USER>() as u32 || length > 65536 {
        return Err(Error::Denied);
    }
    let mut buffer = vec![0usize; (length as usize).div_ceil(std::mem::size_of::<usize>())];
    // SAFETY: aligned buffer is at least length bytes; token is retained.
    if unsafe {
        GetTokenInformation(
            token.0,
            TokenUser,
            buffer.as_mut_ptr().cast(),
            length,
            &mut length,
        )
    } == 0
    {
        return Err(Error::Denied);
    }
    Ok(buffer)
}
struct Pipe<'a> {
    pipe: Handle,
    process: Handle,
    pid: u32,
    cancelled: &'a AtomicBool,
}
impl<'a> Pipe<'a> {
    fn connect_named(cancelled: &'a AtomicBool, pipe_name: &str) -> Result<Self, Error> {
        let name: Vec<u16> = pipe_name.encode_utf16().chain(Some(0)).collect();
        let deadline = Instant::now() + Duration::from_secs(2);
        let pipe = loop {
            if cancelled.load(Ordering::Acquire) {
                return Err(Error::Cancelled);
            }
            // SAFETY: nul-terminated local path; overlapped and identification-only
            // impersonation prevent the Server from acquiring a delegation token.
            let handle = unsafe {
                CreateFileW(
                    name.as_ptr(),
                    GENERIC_READ | GENERIC_WRITE,
                    0,
                    std::ptr::null(),
                    OPEN_EXISTING,
                    FILE_FLAG_OVERLAPPED | SECURITY_SQOS_PRESENT | SECURITY_IDENTIFICATION,
                    std::ptr::null_mut(),
                )
            };
            if handle != INVALID_HANDLE_VALUE {
                break Handle(handle);
            }
            // SAFETY: thread-local error query after failed CreateFile.
            if unsafe { GetLastError() } != ERROR_PIPE_BUSY || Instant::now() >= deadline {
                return Err(Error::Unavailable);
            }
            std::thread::sleep(Duration::from_millis(20));
        };
        let mode = PIPE_READMODE_MESSAGE;
        let mut pid = 0;
        // SAFETY: retained pipe and valid output pointers. Message reads reject trailing bytes.
        if unsafe { SetNamedPipeHandleState(pipe.0, &mode, std::ptr::null(), std::ptr::null()) }
            == 0
            || unsafe { GetNamedPipeServerProcessId(pipe.0, &mut pid) } == 0
        {
            return Err(Error::Denied);
        }
        // SAFETY: query-only process open; the handle prevents PID recycling from authenticating a new process.
        let raw = unsafe {
            OpenProcess(
                PROCESS_QUERY_LIMITED_INFORMATION | PROCESS_SYNCHRONIZE,
                0,
                pid,
            )
        };
        if raw.is_null() {
            return Err(Error::Denied);
        }
        let process = Handle(raw);
        let (mut remote_session, mut local_session) = (0, 0);
        // SAFETY: valid output pointers; current PID is provided by the OS.
        if unsafe { ProcessIdToSessionId(pid, &mut remote_session) } == 0
            || unsafe { ProcessIdToSessionId(GetCurrentProcessId(), &mut local_session) } == 0
            || remote_session != local_session
        {
            return Err(Error::Denied);
        }
        let remote = user(&token(process.0)?)?;
        // SAFETY: current-process pseudo handle is borrowed, never wrapped/closed.
        let local = user(&token(unsafe { GetCurrentProcess() })?)?;
        // SAFETY: GetTokenInformation supplied TOKEN_USER values in aligned,
        // retained buffers, and their SID pointers remain valid during EqualSid.
        let same_user = unsafe {
            let remote = &*remote.as_ptr().cast::<TOKEN_USER>();
            let local = &*local.as_ptr().cast::<TOKEN_USER>();
            EqualSid(remote.User.Sid, local.User.Sid) != 0
        };
        if !same_user {
            return Err(Error::Denied);
        }
        let result = Self {
            pipe,
            process,
            pid,
            cancelled,
        };
        result.peer()?;
        Ok(result)
    }
    fn peer(&self) -> Result<(), Error> {
        let mut pid = 0;
        // SAFETY: both handles remain owned by this worker.
        if unsafe { WaitForSingleObject(self.process.0, 0) } != WAIT_TIMEOUT
            || unsafe { GetNamedPipeServerProcessId(self.pipe.0, &mut pid) } == 0
            || pid != self.pid
        {
            return Err(Error::Denied);
        }
        Ok(())
    }
    fn io(&self, buffer: &mut [u8], read: bool) -> Result<usize, Error> {
        self.peer()?;
        if self.cancelled.load(Ordering::Acquire) {
            return Err(Error::Cancelled);
        }
        // SAFETY: manual-reset event has no borrowed security descriptor/name.
        let raw = unsafe { CreateEventW(std::ptr::null(), 1, 0, std::ptr::null()) };
        if raw.is_null() {
            return Err(Error::Unavailable);
        }
        let event = Handle(raw);
        // SAFETY: all-zero OVERLAPPED is the documented initial state.
        let mut operation: OVERLAPPED = unsafe { std::mem::zeroed() };
        operation.hEvent = event.0;
        let mut count = 0;
        let length = u32::try_from(buffer.len()).map_err(|_| Error::Invalid)?;
        // SAFETY: buffer, operation and event remain alive/unmoved until completion.
        let complete = unsafe {
            if read {
                ReadFile(
                    self.pipe.0,
                    buffer.as_mut_ptr(),
                    length,
                    &mut count,
                    &mut operation,
                )
            } else {
                WriteFile(
                    self.pipe.0,
                    buffer.as_ptr(),
                    length,
                    &mut count,
                    &mut operation,
                )
            }
        };
        // SAFETY: error read immediately after I/O submission.
        if complete == 0 && unsafe { GetLastError() } != ERROR_IO_PENDING {
            return Err(Error::Unavailable);
        }
        if complete == 0 {
            let deadline = Instant::now() + Duration::from_secs(12);
            loop {
                let cancelled = self.cancelled.load(Ordering::Acquire);
                if cancelled || Instant::now() >= deadline || self.peer().is_err() {
                    // SAFETY: cancellation is followed by a blocking drain; no
                    // stack/buffer/event may be freed while the kernel uses it.
                    unsafe {
                        CancelIoEx(self.pipe.0, &operation);
                        GetOverlappedResult(self.pipe.0, &operation, &mut count, 1);
                    }
                    return Err(if cancelled {
                        Error::Cancelled
                    } else {
                        Error::Unavailable
                    });
                }
                // SAFETY: retained event; short slices allow prompt cancellation.
                let ready = unsafe { WaitForSingleObject(event.0, 20) };
                if ready == WAIT_TIMEOUT {
                    continue;
                }
                // SAFETY: drain even an unexpected event failure before releasing storage.
                if ready != WAIT_OBJECT_0 {
                    unsafe {
                        CancelIoEx(self.pipe.0, &operation);
                        GetOverlappedResult(self.pipe.0, &operation, &mut count, 1);
                    }
                    return Err(Error::Unavailable);
                }
                // SAFETY: signalled completion; false includes oversized messages (ERROR_MORE_DATA).
                if unsafe { GetOverlappedResult(self.pipe.0, &operation, &mut count, 0) } == 0 {
                    return Err(Error::Unavailable);
                }
                break;
            }
        }
        self.peer()?;
        if self.cancelled.load(Ordering::Acquire) {
            return Err(Error::Cancelled);
        }
        Ok(count as usize)
    }
}
impl Transport for Pipe<'_> {
    fn exchange(&mut self, request: &[u8]) -> Result<Vec<u8>, Error> {
        let mut request = request.to_vec();
        if self.io(&mut request, false)? != request.len() {
            return Err(Error::Unavailable);
        }
        let mut response = vec![0; protocol::MAX_REPLY];
        let count = self.io(&mut response, true)?;
        if count == 0 {
            return Err(Error::Unavailable);
        }
        response.truncate(count);
        Ok(response)
    }
}

pub fn recognize(
    language: &str,
    generation: u64,
    stopped: &AtomicBool,
    cancelled: &AtomicBool,
    update: impl FnMut(&Update),
) -> Result<String, Error> {
    let mut pipe = Pipe::connect_named(cancelled, PIPE_NAME)?;
    // SAFETY: identity query has no preconditions. Only controller identity is sent, never TSF identity.
    let controller =
        (u64::from(unsafe { GetCurrentProcessId() }) << 32) | (generation & 0xffff_ffff);
    protocol::recognize(&mut pipe, controller, language, stopped, cancelled, update)
}
