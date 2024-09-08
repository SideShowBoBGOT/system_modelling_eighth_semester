use std::fmt::Debug;
use std::ops::{Div, Mul };
use derive_more::{Sub, Add, AddAssign};

pub mod delay_gen {
    use rand::distributions::Distribution;
    use crate::TimeSpan;
    use rand::thread_rng;
    use rand_distr::{Exp, Normal, Uniform};

    #[derive(Clone, Copy, Debug)]
    pub enum DelayGen {
        Normal(Normal<f64>),
        Uniform(Uniform<f64>),
        Exponential(Exp<f64>),
    }

    impl DelayGen {
        pub fn sample(&self) -> TimeSpan {
            TimeSpan(
                match self {
                    Self::Normal(dist) => dist.sample(&mut thread_rng()).round() as u64,
                    Self::Uniform(dist) => dist.sample(&mut thread_rng()).round() as u64,
                    Self::Exponential(dist) => dist.sample(&mut thread_rng()).round() as u64,
                }
            )
        }
    }
}

#[derive(Debug, Copy, Clone, Default, Ord, PartialOrd, Eq, PartialEq)]
struct TimePoint(u64);

impl Sub for TimePoint {
    type Output = TimeSpan;
    fn sub(self, rhs: TimePoint) -> TimeSpan {
        TimeSpan(self.0 - rhs.0)
    }
}

impl Add<TimeSpan> for TimePoint {
    type Output = TimePoint;
    fn add(self, rhs: TimeSpan) -> TimePoint {
        TimePoint(self.0 + rhs.0)
    }
}

#[derive(Debug, Copy, Clone, Default, AddAssign)]
struct TimeSpan(u64);

#[derive(Debug, Copy, Clone, Default, Ord, PartialOrd, Eq, PartialEq, Sub)]
struct QueueSize(u64);
impl QueueSize {
    fn increment(&mut self) {
        self.0 += 1;
    }
    fn decrement(&mut self) {
        self.0 -= 1;
    }
}

impl Mul<TimeSpan> for QueueSize {
    type Output = QueueTimeDur;

    fn mul(self, rhs: TimeSpan) -> Self::Output {
        QueueTimeDur(self.0 * rhs.0)
    }
}

#[derive(Debug, Copy, Clone, Default, Add, AddAssign)]
struct QueueTimeDur(u64);

#[derive(Debug, Copy, Clone, Default)]
struct MeanQueueSize(f64);

impl Div<TimeSpan> for QueueTimeDur {
    type Output = MeanQueueSize;

    fn div(self, rhs: TimeSpan) -> Self::Output {
        MeanQueueSize(self.0 as f64 / rhs.0 as f64)
    }
}

#[derive(Debug, Clone, Copy, Default)]
enum SimulationDelay {
    #[default]
    Failed,
    Processed {
        work_dur: TimeSpan,
        items_processed: u64,
        failures_count: u64,
        queue_time_dur: QueueTimeDur
    }
}

impl Add<TimeSpan> for TimeSpan {
    type Output = TimeSpan;

    fn add(self, rhs: Self) -> Self::Output {
        Self(self.0 + rhs.0)
    }
}

#[derive(Debug)]
struct Payload();

// type ElementNode = Rc<RefCell<dyn Element>>;
//
// fn simulate_model(root: &mut dyn Element, dur: TimeSpan) {
//     let mut current_time = TimePoint::default();
//     let mut stop_time = current_time + dur;
//     while current_time < stop_time {
//         let simulation_delay = root.simulate(Some(Payload{}), current_time);
//     }
// }

mod payload_vec {
    use crate::Payload;

    #[derive(Debug, Default)]
    pub struct PayloadVec {
        count: usize,
    }

    impl PayloadVec {
        pub fn pop(&mut self) -> Option<Payload> {
            if self.count == 0 {
                None
            } else {
                self.count -= 1;
                Some(Payload())
            }
        }

        pub fn push(&mut self) {
            self.count += 1;
        }

        pub fn is_empty(&self) -> bool {
            self.count == 0
        }
    }
}

mod element_processor {
    use crate::{QueueSize, QueueTimeDur, TimePoint, TimeSpan};
    use crate::delay_gen::DelayGen;
    use crate::payload_vec::PayloadVec;

    #[derive(Debug)]
    struct PayloadStats {
        finish_time: TimePoint,
        work_time: TimeSpan
    }

    #[derive(Debug)]
    pub struct BankWindow {
        name: &'static str,
        max_queue_size: QueueSize,
        delay_gen: DelayGen,

        current_payload_stats: Option<PayloadStats>,
        finished_payloads_count: usize,
        interval_between_finished_payloads_sum: TimeSpan,
        last_payload_finished_time: TimePoint,
        work_dur: TimeSpan,

        queue_size: QueueSize,
        queue_time_dur: QueueTimeDur,
        last_simulate_time_point: TimePoint,
    }

    struct UpdateStatsAfterSimulation<'a> {
        current_time: TimePoint,
        queue_size: QueueSize,
        queue_time_dur: &'a mut QueueTimeDur,
        last_simulate_time_point: &'a mut TimePoint
    }

    impl Drop for UpdateStatsAfterSimulation<'_> {
        fn drop(&mut self) {
            let delay = self.current_time - *self.last_simulate_time_point;
            *self.queue_time_dur = self.queue_size * delay;
            *self.last_simulate_time_point = self.current_time;
        }
    }

    // bad name, but self-explanatory
    #[derive(Debug)]
    pub struct PayloadVecWithPossiblySomeConsumeElements(PayloadVec);

    impl BankWindow {
        fn catch_up_with_time(&mut self, current_time: TimePoint) -> UpdateStatsAfterSimulation {
            let _ = &self.delay_gen;
            loop {
                if let Some(current_payload_stats) = &mut self.current_payload_stats {
                    if current_payload_stats.finish_time > current_time {
                        self.last_simulate_time_point = current_time;
                        break;
                    }
                    self.finished_payloads_count += 1;
                    {
                        let interval = current_payload_stats.finish_time - self.last_payload_finished_time;
                        self.interval_between_finished_payloads_sum += interval;
                        self.queue_time_dur += self.queue_size * interval;
                    }
                    self.last_simulate_time_point = current_payload_stats.finish_time;
                    self.last_payload_finished_time = current_payload_stats.finish_time;
                    self.work_dur += current_payload_stats.work_time;

                    if self.queue_size > QueueSize(0) {
                        self.queue_size.decrement();
                        current_payload_stats.work_time = self.delay_gen.sample();
                        current_payload_stats.finish_time = current_time + current_payload_stats.work_time;
                    } else {
                        self.current_payload_stats = None;
                    }
                } else {
                    break;
                }
            }
            UpdateStatsAfterSimulation{
                current_time,
                queue_size: self.queue_size,
                queue_time_dur: &mut self.queue_time_dur,
                last_simulate_time_point: &mut self.last_simulate_time_point
            }
        }

        pub fn simulate_without_payload(&mut self, current_time: TimePoint) {
            let _ = self.catch_up_with_time(current_time);
        }

        pub fn simulate_with_payload(
            &mut self, current_time: TimePoint, mut payload_vec: PayloadVec,
        ) -> PayloadVecWithPossiblySomeConsumeElements {
            let _ = self.catch_up_with_time(current_time);
            loop {
                if self.current_payload_stats.is_none() {
                    if payload_vec.pop().is_none() {
                        break;
                    }
                    let delay = self.delay_gen.sample();
                    self.current_payload_stats = Some(PayloadStats{
                        finish_time: current_time + delay, work_time: delay
                    });
                    continue;
                }
                if self.queue_size < self.max_queue_size {
                    if payload_vec.pop().is_none() {
                        break;
                    }
                    self.queue_size.increment();
                    continue;
                }
                break;
            }
            PayloadVecWithPossiblySomeConsumeElements(payload_vec)
        }


        pub fn get_queue_size(&self) -> QueueSize {
            self.queue_size
        }
        pub fn get_queue_size_mut(&mut self) -> &mut QueueSize {
            &mut self.queue_size
        }
    }
}

mod bank_window_queue_balancer {
    use std::ops::Sub;
    use derive_more::AddAssign;
    use crate::element_processor::BankWindow;
    use crate::{QueueSize, TimePoint};
    use crate::payload_vec::PayloadVec;

    const BANK_WINDOW_MAX_QUEUE_SIZE: QueueSize = QueueSize(3);
    const QUEUE_CHANGE: QueueSize = QueueSize(2);

    struct BankWindowQueueBalancer {
        first_window: BankWindow,
        second_window: BankWindow,
        rejected_count: usize
    }

    fn is_bank_window_full(bank_window: &BankWindow) -> bool {
        BANK_WINDOW_MAX_QUEUE_SIZE == bank_window.get_queue_size()
    }

    #[derive(Debug, AddAssign)]
    struct RebalancedCount(usize);

    fn make_rebalanced_queues(
        first_queue_size: &mut QueueSize,
        second_queue_size: &mut QueueSize
    ) -> RebalancedCount {
        let mmin = first_queue_size.min(second_queue_size);
        let mmax = first_queue_size.max(second_queue_size);
        let mut rebalancing_result = RebalancedCount(0);
        while *mmax - *mmin >= QUEUE_CHANGE {
            mmax.decrement();
            mmin.increment();
            *rebalancing_result += RebalancedCount(1);
        }
        rebalancing_result
    }

    impl BankWindowQueueBalancer {
        fn simulate(&mut self, current_time: TimePoint, mut payload_vec: PayloadVec) {
            make_rebalanced_queues(
                self.first_window.get_queue_size_mut(),
                self.second_window.get_queue_size_mut()
            );
            loop {
                if is_bank_window_full(&self.first_window)
                    && is_bank_window_full(&self.second_window) {
                    while payload_vec.pop().is_some() {
                        self.rejected_count += 1;
                    }
                    break;
                }
                if let Some(payload) = payload_vec.pop() {
                    make_rebalanced_queues(
                        self.first_window.get_queue_size_mut(),
                        self.second_window.get_queue_size_mut()
                    );



                } else {
                    break;
                }
            }
        }
    }
}

mod element_create {
    use crate::delay_gen::DelayGen;
    use crate::payload_vec::PayloadVec;
    use crate::TimePoint;

    pub struct ElementCreate {
        delay_gen: DelayGen,
        next_t: TimePoint,
    }

    impl ElementCreate {
        pub fn simulate(&mut self, current_t: TimePoint) -> PayloadVec {
            let mut payload_vec = PayloadVec::default();
            while self.next_t < current_t {
                self.next_t = self.next_t + self.delay_gen.sample();
                payload_vec.push();
            }
            payload_vec
        }
    }
}

fn main() {

}

#[cfg(test)]
mod tests {
    #[test]
    fn test_bank() {

    }
}


