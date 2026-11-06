//! The TCP ports a thread's programs are listening on (the browser rail, Design 12): every
//! listening socket owned by one of the thread's processes (its CLI, its shells) or anything
//! they started. Windows reads its process list and TCP tables directly; elsewhere this finds
//! nothing yet.
use serde::Serialize;

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Port {
    pub port: u16,
    pub pid: u32,
    /// The program's name, e.g. `node.exe`.
    pub program: String,
}

/// Listening ports owned by `roots` or their descendants, by port number.
pub fn listening(roots: &[u32]) -> Vec<Port> {
    if roots.is_empty() {
        return Vec::new();
    }
    let procs = imp::processes();
    // Every process that descends from one of the roots.
    let mut family: std::collections::HashSet<u32> = roots.iter().copied().collect();
    loop {
        let before = family.len();
        for (pid, parent, _) in &procs {
            if family.contains(parent) {
                family.insert(*pid);
            }
        }
        if family.len() == before {
            break;
        }
    }
    let name = |pid: u32| procs.iter().find(|(p, _, _)| *p == pid).map(|(_, _, n)| n.clone()).unwrap_or_default();
    let mut out: Vec<Port> = imp::listeners()
        .into_iter()
        .filter(|(_, pid)| family.contains(pid))
        .map(|(port, pid)| Port { port, pid, program: name(pid) })
        .collect();
    out.sort_by_key(|p| p.port);
    out.dedup_by_key(|p| p.port);
    out
}

#[cfg(windows)]
mod imp {
    use windows_sys::Win32::Foundation::{CloseHandle, INVALID_HANDLE_VALUE};
    use windows_sys::Win32::NetworkManagement::IpHelper::{
        GetExtendedTcpTable, MIB_TCP6TABLE_OWNER_PID, MIB_TCPTABLE_OWNER_PID, TCP_TABLE_OWNER_PID_LISTENER,
    };
    use windows_sys::Win32::System::Diagnostics::ToolHelp::{
        CreateToolhelp32Snapshot, PROCESSENTRY32W, Process32FirstW, Process32NextW, TH32CS_SNAPPROCESS,
    };

    /// (pid, parent pid, program name) of every process.
    pub fn processes() -> Vec<(u32, u32, String)> {
        let mut out = Vec::new();
        // SAFETY: the snapshot handle is checked and closed; the entry is sized as the API asks.
        unsafe {
            let snap = CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0);
            if snap == INVALID_HANDLE_VALUE {
                return out;
            }
            let mut e: PROCESSENTRY32W = std::mem::zeroed();
            e.dwSize = std::mem::size_of::<PROCESSENTRY32W>() as u32;
            let mut ok = Process32FirstW(snap, &mut e);
            while ok != 0 {
                let len = e.szExeFile.iter().position(|c| *c == 0).unwrap_or(e.szExeFile.len());
                out.push((e.th32ProcessID, e.th32ParentProcessID, String::from_utf16_lossy(&e.szExeFile[..len])));
                ok = Process32NextW(snap, &mut e);
            }
            CloseHandle(snap);
        }
        out
    }

    /// Reads one of Windows' TCP tables into a buffer big enough for it.
    fn table(family: u32) -> Vec<u8> {
        let mut size = 0u32;
        // SAFETY: the first call only asks for the size; the second fills a buffer of that size.
        unsafe {
            GetExtendedTcpTable(std::ptr::null_mut(), &mut size, 0, family, TCP_TABLE_OWNER_PID_LISTENER, 0);
            let mut buf = vec![0u8; size as usize + 1024];
            let mut size = buf.len() as u32;
            if GetExtendedTcpTable(buf.as_mut_ptr().cast(), &mut size, 0, family, TCP_TABLE_OWNER_PID_LISTENER, 0) != 0 {
                return Vec::new();
            }
            buf
        }
    }

    /// (port, owning pid) of every listening TCP socket, IPv4 and IPv6.
    pub fn listeners() -> Vec<(u16, u32)> {
        let mut out = Vec::new();
        let v4 = table(2);
        if v4.len() >= 4 {
            // SAFETY: the buffer holds a MIB_TCPTABLE_OWNER_PID as Windows wrote it.
            unsafe {
                let t = &*(v4.as_ptr() as *const MIB_TCPTABLE_OWNER_PID);
                let rows = std::slice::from_raw_parts(t.table.as_ptr(), t.dwNumEntries as usize);
                out.extend(rows.iter().map(|r| (u16::from_be(r.dwLocalPort as u16), r.dwOwningPid)));
            }
        }
        let v6 = table(23);
        if v6.len() >= 4 {
            // SAFETY: as above, for the IPv6 table.
            unsafe {
                let t = &*(v6.as_ptr() as *const MIB_TCP6TABLE_OWNER_PID);
                let rows = std::slice::from_raw_parts(t.table.as_ptr(), t.dwNumEntries as usize);
                out.extend(rows.iter().map(|r| (u16::from_be(r.dwLocalPort as u16), r.dwOwningPid)));
            }
        }
        out
    }
}

#[cfg(not(windows))]
mod imp {
    pub fn processes() -> Vec<(u32, u32, String)> {
        Vec::new()
    }
    pub fn listeners() -> Vec<(u16, u32)> {
        Vec::new()
    }
}

#[cfg(test)]
mod tests {
    #[cfg(windows)]
    #[test]
    fn finds_a_port_this_process_listens_on() {
        let l = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let port = l.local_addr().unwrap().port();
        let found = super::listening(&[std::process::id()]);
        assert!(found.iter().any(|p| p.port == port), "{port} not in {found:?}");
        assert!(super::listening(&[]).is_empty());
    }
}
