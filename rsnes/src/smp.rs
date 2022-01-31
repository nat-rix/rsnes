use crate::{
    backend::AudioBackend as Backend,
    spc700::Spc700,
    timing::{Cycles, APU_CPU_TIMING_PROPORTION_NTSC, APU_CPU_TIMING_PROPORTION_PAL},
};
use save_state_macro::InSaveState;
use std::sync::mpsc::{channel, Receiver, RecvError, Sender};

#[derive(Debug, Clone)]
enum Action {
    WriteInputPort { addr: u8, data: u8 },
    ReadOutputPort { addr: u8 },
}

#[derive(Debug, Clone)]
enum ThreadCommand {
    RunCycles {
        cycles: Cycles,
        action: Option<Action>,
    },
    KillMe,
}

type ReturnType = Result<(), RecvError>;

#[derive(Debug)]
struct Thread {
    join_handle: Option<std::thread::JoinHandle<ReturnType>>,
    send: Sender<ThreadCommand>,
    recv: Receiver<u8>,
}

#[derive(Debug, InSaveState)]
pub struct Smp<B: Backend> {
    pub spc: Option<Spc700>,
    #[except((|_v, _s| ()), (|_v, _s| ()))]
    pub backend: Option<B>,
    #[except((|_v, _s| ()), (|_v, _s| ()))]
    thread: Option<Thread>,
    timing_proportion: (Cycles, Cycles),
    master_cycles: Cycles,
}

fn threaded_spc<B: Backend>(
    mut spc: Spc700,
    mut backend: B,
    send: Sender<u8>,
    recv: Receiver<ThreadCommand>,
) -> ReturnType {
    loop {
        match recv.recv()? {
            ThreadCommand::RunCycles { cycles, action } => {
                // synchronize
                for _ in 0..cycles {
                    if let Some(sample) = spc.run_cycle() {
                        backend.push_sample(sample)
                    }
                }
                // run action
                match action {
                    Some(Action::WriteInputPort { addr, data }) => {
                        spc.input[usize::from(addr & 3)] = data
                    }
                    Some(Action::ReadOutputPort { addr }) => {
                        let _ = send.send(spc.output[usize::from(addr & 3)]);
                    }
                    None => (),
                }
            }
            ThreadCommand::KillMe => break Ok(()),
        }
    }
}

impl<B: Backend> Smp<B> {
    pub fn new(backend: B, is_pal: bool, is_threaded: bool) -> Self {
        let spc = Spc700::default();
        let timing_proportion = if is_pal {
            APU_CPU_TIMING_PROPORTION_PAL
        } else {
            APU_CPU_TIMING_PROPORTION_NTSC
        };
        if is_threaded {
            let ((m_send, m_recv), (t_send, t_recv)) = (channel(), channel());
            let handle = std::thread::spawn(move || threaded_spc(spc, backend, m_send, t_recv));
            let thread = Some(Thread {
                join_handle: Some(handle),
                send: t_send,
                recv: m_recv,
            });
            Self {
                spc: None,
                backend: None,
                thread,
                timing_proportion,
                master_cycles: 0,
            }
        } else {
            Self {
                spc: Some(spc),
                backend: Some(backend),
                thread: None,
                timing_proportion,
                master_cycles: 0,
            }
        }
    }

    /// Tick in main CPU master cycles
    pub fn tick(&mut self, n: u16) {
        self.master_cycles += Cycles::from(n) * self.timing_proportion.1;
    }

    fn refresh_counters(&mut self) -> Cycles {
        let cycles = self.master_cycles / self.timing_proportion.0;
        self.master_cycles %= self.timing_proportion.0;
        cycles
    }

    fn refresh_no_thread(spc: &mut Spc700, backend: &mut B, cycles: Cycles) {
        for _ in 0..cycles {
            if let Some(sample) = spc.run_cycle() {
                backend.push_sample(sample)
            }
        }
    }

    pub fn refresh(&mut self) {
        let cycles = self.refresh_counters();
        if let (Some(spc), Some(backend)) = (&mut self.spc, &mut self.backend) {
            Self::refresh_no_thread(spc, backend, cycles)
        } else if let Some(thread) = &mut self.thread {
            let _ = thread.send.send(ThreadCommand::RunCycles {
                cycles,
                action: None,
            });
        } else {
            unreachable!()
        }
    }

    pub fn read_output_port(&mut self, addr: u8) -> u8 {
        let cycles = self.refresh_counters();
        if let (Some(spc), Some(backend)) = (&mut self.spc, &mut self.backend) {
            Self::refresh_no_thread(spc, backend, cycles);
            spc.output[usize::from(addr & 3)]
        } else if let Some(thread) = &mut self.thread {
            let _ = thread.send.send(ThreadCommand::RunCycles {
                cycles,
                action: Some(Action::ReadOutputPort { addr }),
            });
            // TODO: dont unwrap, make it more elegant
            thread.recv.recv().unwrap()
        } else {
            unreachable!()
        }
    }

    pub fn write_input_port(&mut self, addr: u8, data: u8) {
        let cycles = self.refresh_counters();
        if let (Some(spc), Some(backend)) = (&mut self.spc, &mut self.backend) {
            Self::refresh_no_thread(spc, backend, cycles);
            spc.input[usize::from(addr & 3)] = data
        } else if let Some(thread) = &mut self.thread {
            let _ = thread.send.send(ThreadCommand::RunCycles {
                cycles,
                action: Some(Action::WriteInputPort { addr, data }),
            });
        } else {
            unreachable!()
        }
    }

    pub fn is_threaded(&self) -> bool {
        self.thread.is_some()
    }
}

impl<B: Backend> Drop for Smp<B> {
    fn drop(&mut self) {
        if let Some(thread) = &mut self.thread {
            drop(thread.send.send(ThreadCommand::KillMe));
            if let Some(Ok(Err(err))) = thread.join_handle.take().map(|t| t.join()) {
                todo!("throw useful error ({})", err)
            }
        }
    }
}
