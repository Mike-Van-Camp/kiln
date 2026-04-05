//! Debugger data model: session state, breakpoints, registers, call stack, and memory watches
//! (Sprint 18).

/// Current state of the debug session.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum DebuggerState {
    #[default]
    Disconnected,
    Running,
    Paused,
    Exited,
}

/// A breakpoint set at a specific address.
#[derive(Debug, Clone)]
pub struct Breakpoint {
    pub id: u32,
    pub address: u64,
    pub enabled: bool,
    pub hit_count: u32,
    pub condition: Option<String>,
}

/// A CPU register name/value pair.
#[derive(Debug, Clone)]
pub struct RegisterValue {
    pub name: String,
    pub value: u64,
    /// Register width in bytes (e.g. 4 for 32-bit, 8 for 64-bit).
    pub size: u8,
}

/// A single frame in the call stack.
#[derive(Debug, Clone)]
pub struct StackFrame {
    pub index: u32,
    pub address: u64,
    pub function_name: Option<String>,
    pub file: Option<String>,
    pub line: Option<u32>,
}

/// A watched memory region.
#[derive(Debug, Clone)]
pub struct MemoryWatch {
    pub address: u64,
    pub size: usize,
    pub label: String,
    pub data: Vec<u8>,
}

/// Holds all state for an active debug session.
#[derive(Debug, Default)]
pub struct DebugSession {
    pub state: DebuggerState,
    pub breakpoints: Vec<Breakpoint>,
    pub registers: Vec<RegisterValue>,
    pub call_stack: Vec<StackFrame>,
    pub watches: Vec<MemoryWatch>,
    pub current_address: Option<u64>,
    next_bp_id: u32,
    pub output_log: Vec<String>,
}

impl DebugSession {
    pub fn new() -> Self {
        Self::default()
    }

    /// Add a breakpoint at `addr` and return its id.
    pub fn add_breakpoint(&mut self, addr: u64) -> u32 {
        self.next_bp_id += 1;
        self.breakpoints.push(Breakpoint {
            id: self.next_bp_id,
            address: addr,
            enabled: true,
            hit_count: 0,
            condition: None,
        });
        self.output_log.push(format!(
            "Breakpoint {} set at 0x{:x}",
            self.next_bp_id, addr
        ));
        self.next_bp_id
    }

    /// Remove the breakpoint with the given id.
    pub fn remove_breakpoint(&mut self, id: u32) {
        self.breakpoints.retain(|bp| bp.id != id);
        self.output_log.push(format!("Breakpoint {id} removed"));
    }

    /// Toggle the enabled state of a breakpoint.
    pub fn toggle_breakpoint(&mut self, id: u32) {
        if let Some(bp) = self.breakpoints.iter_mut().find(|bp| bp.id == id) {
            bp.enabled = !bp.enabled;
            let state = if bp.enabled { "enabled" } else { "disabled" };
            self.output_log.push(format!("Breakpoint {id} {state}"));
        }
    }

    /// Remove all breakpoints.
    pub fn clear_breakpoints(&mut self) {
        self.breakpoints.clear();
        self.output_log.push("All breakpoints cleared".into());
    }

    /// Returns `true` if there is an enabled breakpoint at `addr`.
    pub fn has_breakpoint_at(&self, addr: u64) -> bool {
        self.breakpoints
            .iter()
            .any(|bp| bp.address == addr && bp.enabled)
    }

    /// Add a memory watch region.
    pub fn add_watch(&mut self, address: u64, size: usize, label: String) {
        self.watches.push(MemoryWatch {
            address,
            size,
            label: label.clone(),
            data: vec![0; size],
        });
        self.output_log.push(format!(
            "Watch \"{label}\" added at 0x{address:x} ({size} bytes)"
        ));
    }

    /// Remove a memory watch by index.
    pub fn remove_watch(&mut self, index: usize) {
        if index < self.watches.len() {
            let label = self.watches[index].label.clone();
            self.watches.remove(index);
            self.output_log.push(format!("Watch \"{label}\" removed"));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_add_remove_breakpoint() {
        let mut session = DebugSession::new();
        let id = session.add_breakpoint(0x1000);
        assert_eq!(id, 1);
        assert_eq!(session.breakpoints.len(), 1);
        assert_eq!(session.breakpoints[0].address, 0x1000);
        assert!(session.breakpoints[0].enabled);

        session.remove_breakpoint(id);
        assert!(session.breakpoints.is_empty());
    }

    #[test]
    fn test_toggle_breakpoint() {
        let mut session = DebugSession::new();
        let id = session.add_breakpoint(0x2000);
        assert!(session.breakpoints[0].enabled);

        session.toggle_breakpoint(id);
        assert!(!session.breakpoints[0].enabled);

        session.toggle_breakpoint(id);
        assert!(session.breakpoints[0].enabled);
    }

    #[test]
    fn test_clear_breakpoints() {
        let mut session = DebugSession::new();
        session.add_breakpoint(0x1000);
        session.add_breakpoint(0x2000);
        session.add_breakpoint(0x3000);
        assert_eq!(session.breakpoints.len(), 3);

        session.clear_breakpoints();
        assert!(session.breakpoints.is_empty());
    }

    #[test]
    fn test_has_breakpoint_at() {
        let mut session = DebugSession::new();
        let id = session.add_breakpoint(0x1000);
        assert!(session.has_breakpoint_at(0x1000));
        assert!(!session.has_breakpoint_at(0x2000));

        // Disabled breakpoints should not count
        session.toggle_breakpoint(id);
        assert!(!session.has_breakpoint_at(0x1000));
    }

    #[test]
    fn test_add_remove_watch() {
        let mut session = DebugSession::new();
        session.add_watch(0x4000, 16, "stack".into());
        assert_eq!(session.watches.len(), 1);
        assert_eq!(session.watches[0].address, 0x4000);
        assert_eq!(session.watches[0].size, 16);

        session.remove_watch(0);
        assert!(session.watches.is_empty());
    }

    #[test]
    fn test_default_state() {
        let session = DebugSession::new();
        assert_eq!(session.state, DebuggerState::Disconnected);
        assert!(session.breakpoints.is_empty());
        assert!(session.registers.is_empty());
        assert!(session.call_stack.is_empty());
        assert!(session.watches.is_empty());
        assert!(session.current_address.is_none());
    }

    #[test]
    fn test_breakpoint_ids_increment() {
        let mut session = DebugSession::new();
        let id1 = session.add_breakpoint(0x1000);
        let id2 = session.add_breakpoint(0x2000);
        let id3 = session.add_breakpoint(0x3000);
        assert_eq!(id1, 1);
        assert_eq!(id2, 2);
        assert_eq!(id3, 3);
    }

    #[test]
    fn test_output_log_records_operations() {
        let mut session = DebugSession::new();
        let id = session.add_breakpoint(0x1000);
        session.toggle_breakpoint(id);
        session.remove_breakpoint(id);
        session.clear_breakpoints();
        assert_eq!(session.output_log.len(), 4);
    }
}
