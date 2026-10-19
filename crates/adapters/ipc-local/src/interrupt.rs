//! Native Ctrl-C subscription for CLI engines. Signal callbacks never run on the document thread.
use std::{future::Future, io, sync::mpsc, thread};
use tokio::sync::oneshot;

pub struct InterruptGuard {
    stop: Option<oneshot::Sender<()>>,
    worker: Option<thread::JoinHandle<()>>,
}
impl Drop for InterruptGuard {
    fn drop(&mut self) {
        if let Some(stop) = self.stop.take() {
            let _ = stop.send(());
        }
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}
/// Installs the subscription before returning. Drop it to stop the watcher, without invoking `action`.
pub fn on_interrupt(action: impl FnOnce() + Send + 'static) -> io::Result<InterruptGuard> {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()?;
    let (stop, mut stopped) = oneshot::channel();
    let (ready, started) = mpsc::sync_channel(1);
    let worker=thread::Builder::new().name("vellum-cli-interrupt".into()).spawn(move||{
        runtime.block_on(async move{
            let signal=tokio::signal::ctrl_c();tokio::pin!(signal);
            // First poll registers the OS handler before the CLI sends a model request.
            let mut sent=false;
            let already=std::future::poll_fn(|cx|{
                if !sent {let state=signal.as_mut().poll(cx);let _=ready.send(());sent=true;if state.is_ready(){return std::task::Poll::Ready(true);}}
                std::task::Poll::Ready(false)
            }).await;
            if already {action();return;}
            tokio::select!{ _=&mut stopped=>{}, result=&mut signal=>{if result.is_ok(){action();}} }
        });
    })?;
    started
        .recv()
        .map_err(|_| io::Error::other("Could not install CLI interrupt handler"))?;
    Ok(InterruptGuard {
        stop: Some(stop),
        worker: Some(worker),
    })
}
