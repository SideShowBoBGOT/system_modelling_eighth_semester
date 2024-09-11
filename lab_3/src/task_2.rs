use std::collections::{BinaryHeap, VecDeque};
use rand::distributions::{Distribution};
use rand::{thread_rng, RngCore};
use crate::task_2::create_patient::EventNewPatient;
use crate::task_2::event_lab_registration::EventLabRegistration;
use crate::task_2::event_laboratory::EventLaboratory;
use crate::task_2::event_patient_wards::EventPatientWards;
use crate::task_2::event_reception_department::EventReceptionDepartment;
use crate::task_2::transition_lab_reception::EventTransitionFromLabToReception;
use lazy_static::lazy_static;

#[derive(Debug, Clone, Copy, Ord, PartialOrd, Eq, PartialEq, Default)]
enum Patient {
    #[default]
    Three,
    Two,
    One,
}

#[derive(Debug, Default)]
struct Clinic {
    reception_department: ReceptionDepartment,
    patient_wards: PatientWards,
    lab_registry: LabRegistry,
    laboratory: Laboratory
}

#[derive(Debug, Default)]
struct ReceptionDepartment {
    queue: BinaryHeap<Patient>,
    is_doctor_busy: [bool; 2],
}

#[derive(Debug, Default)]
struct PatientWards {
    queue_size: usize,
    is_attendant_busy: [bool; 3],
}

#[derive(Debug, Default)]
struct LabRegistry {
    queue: VecDeque<Patient>,
    is_busy: bool
}

#[derive(Debug, Default)]
struct Laboratory {
    queue: VecDeque<Patient>,
    is_lab_assistant_busy: [bool; 2],
}

mod create_patient {
    use std::cell::RefCell;
    use std::rc::Rc;
    use lazy_static::lazy_static;
    use rand::distributions::{Distribution, Uniform};
    use rand_distr::Exp;
    use crate::task_2::event_reception_department::EventReceptionDepartment;
    use crate::task_2::{Clinic, Patient};
    use crate::TimeSpan;
    use crate::utils::TimePoint;

    #[derive(Debug, Default)]
    pub struct EventNewPatient {
        current_t: TimePoint,
        patient: Patient,
        clinic: Rc<RefCell<Clinic>>
    }

    lazy_static! {
        static ref DELAY_GEN: Exp<f64> = Exp::new(15.0).expect("Failed to create delay gen");
    }

    fn generate_patient() -> Patient {
        let value = Uniform::new(0.0, 1.0).sample(&mut rand::thread_rng());
        match value {
            ..0.5 => Patient::One,
            0.5..0.6 => Patient::Two,
            0.6.. => Patient::Three,
            _ => panic!("PatientType::generate_patient error")
        }
    }

    impl EventNewPatient {

        pub fn get_current_t(&self) -> TimePoint {
            self.current_t
        }

        pub fn iterate(self) -> (Self, Option<EventReceptionDepartment>) {
            let free_doctor_index = self.clinic.borrow()
                .reception_department.is_doctor_busy.iter().position(|d| !*d);
            let event_reception_dep = if let Some(index) = free_doctor_index {
                assert!(self.clinic.borrow_mut().reception_department.queue.is_empty());
                self.clinic.borrow_mut().reception_department.is_doctor_busy[index] = true;
                Some(EventReceptionDepartment::new(self.current_t, index, self.clinic.clone(), self.patient))
            } else {
                self.clinic.borrow_mut().reception_department.queue.push(self.patient);
                None
            };
            (
                Self {
                    current_t: self.current_t + TimeSpan(DELAY_GEN.sample(&mut rand::thread_rng())),
                    patient: generate_patient(),
                    clinic: self.clinic.clone()
                },
                event_reception_dep,
            )
        }
    }
}

mod event_reception_department {
    use std::cell::RefCell;
    use std::rc::Rc;
    use crate::task_2::{Clinic, Patient};
    use crate::task_2::event_patient_wards::EventPatientWards;
    use crate::task_2::transition_lab_reception::{EventTransitionFromReceptionToLaboratory, EventTransitionReceptionLaboratory};
    use crate::TimeSpan;
    use crate::utils::TimePoint;

    pub struct EventReceptionDepartment {
        current_t: TimePoint,
        doctor_index: usize,
        clinic: Rc<RefCell<Clinic>>,
        patient: Patient
    }

    fn determine_delay(patient: Patient) -> TimeSpan {
        match patient {
            Patient::One => TimeSpan(15.0),
            Patient::Two => TimeSpan(40.0),
            Patient::Three => TimeSpan(30.0),
        }
    }

    enum ReceptionDepartmentTransitionToResult {
        PatientWards(Option<EventPatientWards>),
        FromReceptionToLaboratory(EventTransitionFromReceptionToLaboratory)
    }

    impl EventReceptionDepartment {
        pub fn get_current_t(&self) -> TimePoint {
            self.current_t
        }

        pub fn new(old_current_t: TimePoint, doctor_index: usize, clinic: Rc<RefCell<Clinic>>, patient: Patient) -> Self {
            Self{current_t: old_current_t + determine_delay(patient), doctor_index, clinic, patient}
        }

        pub fn iterate(self) -> (Option<EventReceptionDepartment>, ReceptionDepartmentTransitionToResult) {
            let transition_to = match self.patient {
                Patient::One => ReceptionDepartmentTransitionToResult::PatientWards (
                    {
                        let free_attendant_index = self.clinic.borrow()
                            .patient_wards.is_attendant_busy.iter().position(|d| !*d);
                        if let Some(index) = free_attendant_index {
                            assert_eq!(self.clinic.borrow().patient_wards.queue_size, 0);
                            self.clinic.borrow_mut().patient_wards.is_attendant_busy[index] = true;
                            Some(EventPatientWards::new(self.current_t, self.clinic.clone(), index))
                        } else {
                            self.clinic.borrow_mut().patient_wards.queue_size += 1;
                            None
                        }
                    }
                ),
                Patient::Two | Patient::Three => {
                    ReceptionDepartmentTransitionToResult::FromReceptionToLaboratory({
                        EventTransitionFromReceptionToLaboratory(
                            EventTransitionReceptionLaboratory::new(self.current_t, self.clinic.clone(), self.patient)
                        )
                    })
                },
            };
            let next_reception_dep = {
                let mut clinic = self.clinic.borrow_mut();
                if let Some(patient) = clinic.reception_department.queue.pop() {
                    Some(Self::new(self.current_t, self.doctor_index, self.clinic.clone(), patient))
                } else {
                    clinic.reception_department.is_doctor_busy[self.doctor_index] = false;
                    None
                }
            };
            (next_reception_dep, transition_to)
        }
    }
}


mod transition_lab_reception {
    use std::cell::RefCell;
    use std::rc::Rc;
    use lazy_static::lazy_static;
    use rand::distributions::{Distribution, Uniform};
    use crate::task_2::{Clinic, Patient};
    use crate::task_2::event_lab_registration::EventLabRegistration;
    use crate::task_2::event_reception_department::EventReceptionDepartment;
    use crate::TimeSpan;
    use crate::utils::TimePoint;

    lazy_static! {
        static ref RECEPTION_LABORATORY_TRANSITION_DELAY: Uniform<f64> = Uniform::new(2.0, 5.0);
    }

    pub struct EventTransitionReceptionLaboratory {
        current_t: TimePoint,
        clinic: Rc<RefCell<Clinic>>,
        patient: Patient,
    }

    impl EventTransitionReceptionLaboratory {
        pub fn new(old_current_t: TimePoint, clinic: Rc<RefCell<Clinic>>, patient: Patient) -> Self {
            let delay = TimeSpan(RECEPTION_LABORATORY_TRANSITION_DELAY.sample(&mut rand::thread_rng()));
            Self{current_t: old_current_t + delay, clinic, patient}
        }
    }

    pub struct EventTransitionFromReceptionToLaboratory(pub EventTransitionReceptionLaboratory);

    impl EventTransitionFromReceptionToLaboratory {
        pub fn get_current_t(&self) -> TimePoint {
            self.0.current_t
        }

        pub fn iterate(self) -> Option<EventLabRegistration> {
            if self.0.clinic.borrow().lab_registry.is_busy {
                self.0.clinic.borrow_mut().lab_registry.queue.push_back(self.0.patient);
                None
            } else {
                assert!(self.0.clinic.borrow().lab_registry.queue.is_empty());
                self.0.clinic.borrow_mut().lab_registry.is_busy = true;
                Some(EventLabRegistration::new(self.0.current_t, self.0.clinic, self.0.patient))
            }
        }
    }

    pub struct EventTransitionFromLabToReception(pub EventTransitionReceptionLaboratory);

    impl EventTransitionFromLabToReception {
        pub fn get_current_t(&self) -> TimePoint {
            self.0.current_t
        }

        pub fn iterate(self) -> Option<EventReceptionDepartment> {
            let free_doctor_index = self.0.clinic.borrow()
                .reception_department.is_doctor_busy.iter().position(|d| !*d);
            if let Some(index) = free_doctor_index {
                assert!(self.0.clinic.borrow_mut().reception_department.queue.is_empty());
                self.0.clinic.borrow_mut().reception_department.is_doctor_busy[index] = true;
                Some(EventReceptionDepartment::new(self.0.current_t, index, self.0.clinic.clone(), self.0.patient))
            } else {
                self.0.clinic.borrow_mut().reception_department.queue.push(self.0.patient);
                None
            }
        }
    }

}

mod event_patient_wards {
    use std::cell::RefCell;
    use std::rc::Rc;
    use lazy_static::lazy_static;
    use rand::distributions::{Distribution, Uniform};
    use crate::task_2::{Clinic, EventTerminal};
    use crate::TimeSpan;
    use crate::utils::TimePoint;

    pub struct EventPatientWards {
        current_t: TimePoint,
        clinic: Rc<RefCell<Clinic>>,
        attendant_index: usize
    }

    lazy_static! {
        static ref DELAY_GEN: Uniform<f64> = Uniform::new(3.0, 8.0);
    }

    impl EventPatientWards {
        pub fn get_current_t(&self) -> TimePoint {
            self.current_t
        }

        pub fn new(old_current_t: TimePoint, clinic: Rc<RefCell<Clinic>>, attendant_index: usize) -> Self {
            let delay = TimeSpan(DELAY_GEN.sample(&mut rand::thread_rng()));
            Self{current_t: old_current_t + delay, clinic, attendant_index}
        }

        pub fn iterate(self) -> (Option<Self>, EventTerminal) {
            let mut clinic = self.clinic.borrow_mut();
            let next_event = if clinic.patient_wards.queue_size > 0 {
                clinic.patient_wards.queue_size -= 1;
                Some(Self::new(self.current_t, self.clinic.clone(), self.attendant_index))
            } else {
                clinic.patient_wards.is_attendant_busy[self.attendant_index] = false;
                None
            };
            (next_event, EventTerminal {})
        }
    }
}

struct EventTerminal {}

fn get_erlang_distribution(shape: i64, scale: f64) -> rand_simple::Erlang {
    let mut erlang = rand_simple::Erlang::new(
        [
            thread_rng().next_u32(),
            thread_rng().next_u32(),
            thread_rng().next_u32(),
        ]
    );
    erlang.try_set_params(shape, scale).expect("Erlang set params failed");
    erlang
}

mod event_lab_registration {
    use std::cell::RefCell;
    use std::rc::Rc;
    use crate::task_2::{get_erlang_distribution, Clinic, Patient};
    use crate::task_2::event_laboratory::EventLaboratory;
    use crate::TimeSpan;
    use crate::utils::TimePoint;

    fn sample_delay() -> TimeSpan {
        TimeSpan(get_erlang_distribution(3, 4.5).sample())
    }

    pub struct EventLabRegistration {
        current_t: TimePoint,
        clinic: Rc<RefCell<Clinic>>,
        patient: Patient
    }

    impl EventLabRegistration {
        pub fn get_current_t(&self) -> TimePoint {
            self.current_t
        }

        pub fn new(
            old_current_t: TimePoint,
            clinic: Rc<RefCell<Clinic>>,
            patient: Patient
        ) -> Self {
            Self{current_t: old_current_t + sample_delay(), clinic, patient}
        }

        pub fn iterate(self) -> (Option<Self>, Option<EventLaboratory>) {
            let free_assistant_index = self.clinic.borrow()
                .laboratory.is_lab_assistant_busy.iter().position(|d| !*d);
            let event_lab = if let Some(index) = free_assistant_index {
                assert!(self.clinic.borrow().laboratory.queue.is_empty());
                self.clinic.borrow_mut().laboratory.is_lab_assistant_busy[index] = true;
                Some(EventLaboratory::new(self.current_t, self.clinic.clone(), self.patient, index))
            } else {
                self.clinic.borrow_mut().laboratory.queue.push_back(self.patient);
                None
            };
            let next_reg = {
                let mut clinic = self.clinic.borrow_mut();
                if let Some(patient) = clinic.lab_registry.queue.pop_front() {
                    Some(Self::new(self.current_t, self.clinic.clone(), patient))
                } else {
                    clinic.lab_registry.is_busy = false;
                    None
                }
            };
            (next_reg, event_lab)
        }
    }
}

mod event_laboratory {
    use std::cell::RefCell;
    use std::rc::Rc;
    use crate::task_2::{get_erlang_distribution, Clinic, EventTerminal, Patient};
    use crate::task_2::transition_lab_reception::{EventTransitionFromLabToReception, EventTransitionReceptionLaboratory};
    use crate::TimeSpan;
    use crate::utils::TimePoint;

    pub struct EventLaboratory {
        current_t: TimePoint,
        clinic: Rc<RefCell<Clinic>>,
        patient: Patient,
        assistant_index: usize
    }

    fn sample_delay() -> TimeSpan {
        TimeSpan(get_erlang_distribution(2, 4.0).sample())
    }

    enum EventLaboratoryTransitionResult {
        TransitionFromLabToReception(EventTransitionFromLabToReception),
        Terminal(EventTerminal)
    }

    impl EventLaboratory {
        pub fn get_current_t(&self) -> TimePoint {
            self.current_t
        }

        pub fn new(
            old_current_t: TimePoint,
            clinic: Rc<RefCell<Clinic>>,
            patient: Patient,
            assistant_index: usize
        ) -> Self {
            Self{current_t: old_current_t + sample_delay(), clinic, patient, assistant_index}
        }

        pub fn iterate(self) -> (Option<Self>, EventLaboratoryTransitionResult) {
            let transition_to = match self.patient {
                Patient::One => panic!("Patient one can not be in the laboratory"),
                Patient::Two => EventLaboratoryTransitionResult::TransitionFromLabToReception(
                    EventTransitionFromLabToReception(
                        EventTransitionReceptionLaboratory::new(
                            self.current_t, self.clinic.clone(), Patient::One
                        )
                    )
                ),
                Patient::Three => EventLaboratoryTransitionResult::Terminal(
                    EventTerminal{}
                ),
            };
            let next_event = {
                let mut clinic = self.clinic.borrow_mut();
                if let Some(patient) = clinic.laboratory.queue.pop_front() {
                    Some(Self::new(self.current_t, self.clinic.clone(), patient, self.assistant_index))
                } else {
                    clinic.laboratory.is_lab_assistant_busy[self.assistant_index] = false;
                    None
                }
            };
            (next_event, transition_to)
        }
    }
}

enum Event {
    NewPatient(EventNewPatient),
    ReceptionDepartment(EventReceptionDepartment),
    TransitionFromLabToReception(EventTransitionFromLabToReception),
    TransitionFromReceptionLaboratory(EventTransitionFromLabToReception),
    PatientWards(EventPatientWards),
    LabRegistration(EventLabRegistration),
    Laboratory(EventLaboratory),
    Terminal(EventTerminal),
}

impl Default for Event {
    fn default() -> Self {
        Self::NewPatient(EventNewPatient::default())
    }
}

#[cfg(test)]
mod tests {
    use crate::task_2::Event;

    #[test]
    fn test_general() {

        let mut nodes = vec![Event::default()];

        // let last_event = loop {
        //
        //     nodes.sort_by(|a, b| {
        //         b.get_current_t().partial_cmp(&a.get_current_t())
        //             .expect("Can not compare events current_t")
        //     });
        //
        //     let next_event = nodes.pop().unwrap();
        //     if next_event.get_current_t() > end_time {
        //         break next_event;
        //     }
        //     match next_event {
        //         Event::EventCreate(event) => {
        //             let (event_create, event_process) = event.iterate();
        //             nodes.push(Event::EventCreate(event_create));
        //             if let Some(event_process) = event_process {
        //                 nodes.push(Event::EventProcess(event_process));
        //             }
        //         },
        //         Event::EventProcess(event) => {
        //             if let Some(event_process) = event.iterate() {
        //                 nodes.push(Event::EventProcess(event_process));
        //             }
        //         }
        //     }
        // };

    }

    #[test]
    fn test_binary_heap() {

    }
}