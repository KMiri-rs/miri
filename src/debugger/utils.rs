use crate::{MemoryKind, MiriMemoryKind};

pub fn kind_str(kind: MemoryKind) -> &'static str {
    match kind {
        MemoryKind::Stack => "Stack",
        MemoryKind::CallerLocation => "CallerLoc",
        MemoryKind::Machine(kind) =>
            match kind {
                MiriMemoryKind::Kernel => "Kernel",
                MiriMemoryKind::Rust => "Rust",
                MiriMemoryKind::Miri => "Miri",
                MiriMemoryKind::C => "C",
                MiriMemoryKind::WinHeap => "WinHeap",
                MiriMemoryKind::WinLocal => "WinLocal",
                MiriMemoryKind::Machine => "Machine",
                MiriMemoryKind::Runtime => "Runtime",
                MiriMemoryKind::Global => "Global",
                MiriMemoryKind::ExternStatic => "ExternStatic",
                MiriMemoryKind::Tls => "Tls",
                MiriMemoryKind::Mmap => "Mmap",
            },
    }
}

pub fn hsize(n: impl humansize::ToF64 + humansize::Unsigned) -> String {
    humansize::format_size(n, humansize::BINARY)
}
