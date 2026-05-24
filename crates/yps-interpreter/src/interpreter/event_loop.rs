use std::cmp::Ordering;
use std::collections::{BinaryHeap, HashSet};
use std::time::{Duration, Instant};

use yps_lexer::Span;

use crate::error::RuntimeError;
use crate::value::Value;

use super::Interpreter;

pub(crate) type Macrotask = Box<dyn FnOnce(&mut Interpreter, Span) -> Result<(), RuntimeError>>;

pub(crate) struct ScheduledTask {
    pub deadline: Instant,
    pub seq: u64,
    pub id: u64,
    pub task: Macrotask,
}

impl Eq for ScheduledTask {}
impl PartialEq for ScheduledTask {
    fn eq(&self, other: &Self) -> bool {
        self.deadline == other.deadline && self.seq == other.seq
    }
}
impl Ord for ScheduledTask {
    fn cmp(&self, other: &Self) -> Ordering {
        other.deadline.cmp(&self.deadline).then_with(|| other.seq.cmp(&self.seq))
    }
}
impl PartialOrd for ScheduledTask {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

pub(crate) struct MacrotaskQueue {
    heap: BinaryHeap<ScheduledTask>,
    cancelled: HashSet<u64>,
    next_id: u64,
    next_seq: u64,
}

impl MacrotaskQueue {
    pub fn new() -> Self {
        Self { heap: BinaryHeap::new(), cancelled: HashSet::new(), next_id: 1, next_seq: 0 }
    }

    pub fn schedule(&mut self, delay: Duration, task: Macrotask) -> u64 {
        let id = self.next_id;
        self.next_id = self.next_id.wrapping_add(1);
        let seq = self.next_seq;
        self.next_seq = self.next_seq.wrapping_add(1);
        self.heap.push(ScheduledTask { deadline: Instant::now() + delay, seq, id, task });
        id
    }

    pub fn schedule_with_id(&mut self, id: u64, delay: Duration, task: Macrotask) {
        let seq = self.next_seq;
        self.next_seq = self.next_seq.wrapping_add(1);
        self.heap.push(ScheduledTask { deadline: Instant::now() + delay, seq, id, task });
    }

    pub fn cancel(&mut self, id: u64) {
        self.cancelled.insert(id);
    }

    pub fn allocate_id(&mut self) -> u64 {
        let id = self.next_id;
        self.next_id = self.next_id.wrapping_add(1);
        id
    }

    pub fn is_empty(&self) -> bool {
        self.heap.iter().all(|t| self.cancelled.contains(&t.id))
    }

    pub fn next_deadline(&self) -> Option<Instant> {
        for t in self.heap.iter() {
            if !self.cancelled.contains(&t.id) {
                return Some(t.deadline);
            }
        }
        None
    }

    pub fn pop_ready(&mut self) -> Option<ScheduledTask> {
        while let Some(top) = self.heap.peek() {
            if self.cancelled.contains(&top.id) {
                self.heap.pop();
                continue;
            }
            if top.deadline <= Instant::now() {
                return self.heap.pop();
            }
            return None;
        }
        None
    }

    pub fn pop_next_blocking(&mut self) -> Option<ScheduledTask> {
        loop {
            let next_deadline = self.next_deadline()?;
            let now = Instant::now();
            if next_deadline > now {
                std::thread::sleep(next_deadline - now);
            }
            if let Some(task) = self.pop_ready() {
                return Some(task);
            }
        }
    }
}

impl Default for MacrotaskQueue {
    fn default() -> Self {
        Self::new()
    }
}

fn take_callback(value: Option<Value>, fn_name: &str, span: Span) -> Result<Value, RuntimeError> {
    let v = value.ok_or_else(|| RuntimeError::new(format!("'{fn_name}' ожидает функцию"), span))?;
    if !matches!(v, Value::Function { .. } | Value::BuiltinFunction(_)) {
        return Err(RuntimeError::new(format!("'{fn_name}' ожидает функцию, получено '{}'", v.type_name()), span));
    }
    Ok(v)
}

fn report_async_error(source: &str, err: &RuntimeError) {
    eprintln!("необработанное исключение в '{source}': {}", err.message);
}

fn parse_delay_ms(value: Option<Value>, fn_name: &str, span: Span) -> Result<u64, RuntimeError> {
    match value {
        Some(Value::Number(n)) if n.is_finite() && n >= 0.0 => Ok(n as u64),
        Some(Value::Undefined) | None => Ok(0),
        Some(other) => Err(RuntimeError::new(
            format!("'{fn_name}' ожидает миллисекунды числом, получено '{}'", other.type_name()),
            span,
        )),
    }
}

impl Interpreter {
    pub(crate) fn schedule_macrotask(&mut self, delay: Duration, task: Macrotask) -> u64 {
        self.macrotasks.schedule(delay, task)
    }

    pub(crate) fn schedule_macrotask_with_id(&mut self, id: u64, delay: Duration, task: Macrotask) {
        self.macrotasks.schedule_with_id(id, delay, task);
    }

    pub(crate) fn cancel_macrotask(&mut self, id: u64) {
        self.macrotasks.cancel(id);
    }

    pub(crate) fn allocate_macrotask_id(&mut self) -> u64 {
        self.macrotasks.allocate_id()
    }

    pub(crate) fn try_call_timer_builtin(
        &mut self,
        name: &str,
        args: Vec<Value>,
        span: Span,
    ) -> Option<Result<Value, RuntimeError>> {
        match name {
            "чутка" => Some(self.builtin_schedule_timeout(args, span)),
            "отменаЧутки" | "отменаИнтервала" => Some(self.builtin_cancel_timer(args, span)),
            "интервал" => Some(self.builtin_schedule_interval(args, span)),
            "сразу" => Some(self.builtin_queue_microtask(args, span, false)),
            "наСледующемТике" => Some(self.builtin_queue_microtask(args, span, true)),
            _ => None,
        }
    }

    fn builtin_schedule_timeout(&mut self, args: Vec<Value>, span: Span) -> Result<Value, RuntimeError> {
        let mut it = args.into_iter();
        let cb = take_callback(it.next(), "чутка", span)?;
        let ms = parse_delay_ms(it.next(), "чутка", span)?;
        let id = self.schedule_macrotask(
            Duration::from_millis(ms),
            Box::new(move |interp, sp| {
                if let Err(e) = interp.call_function(cb, vec![], sp) {
                    report_async_error("чутка", &e);
                }
                Ok(())
            }),
        );
        Ok(Value::Number(id as f64))
    }

    fn builtin_cancel_timer(&mut self, args: Vec<Value>, span: Span) -> Result<Value, RuntimeError> {
        let id = match args.into_iter().next() {
            Some(Value::Number(n)) if n.is_finite() && n >= 0.0 => n as u64,
            Some(Value::Undefined) | None => return Ok(Value::Undefined),
            Some(other) => {
                return Err(RuntimeError::new(
                    format!("Идентификатор таймера должен быть числом, получено '{}'", other.type_name()),
                    span,
                ));
            }
        };
        self.cancel_macrotask(id);
        Ok(Value::Undefined)
    }

    fn builtin_schedule_interval(&mut self, args: Vec<Value>, span: Span) -> Result<Value, RuntimeError> {
        let mut it = args.into_iter();
        let cb = take_callback(it.next(), "интервал", span)?;
        let ms = parse_delay_ms(it.next(), "интервал", span)?;
        let id = self.allocate_macrotask_id();
        self.schedule_interval_tick(id, ms, cb);
        Ok(Value::Number(id as f64))
    }

    fn schedule_interval_tick(&mut self, id: u64, ms: u64, cb: Value) {
        let cb_clone = cb.clone();
        self.schedule_macrotask_with_id(
            id,
            Duration::from_millis(ms),
            Box::new(move |interp, sp| {
                if let Err(e) = interp.call_function(cb_clone, vec![], sp) {
                    report_async_error("интервал", &e);
                }
                if !interp.macrotasks.cancelled.contains(&id) {
                    interp.schedule_interval_tick(id, ms, cb);
                }
                Ok(())
            }),
        );
    }

    fn builtin_queue_microtask(&mut self, args: Vec<Value>, span: Span, front: bool) -> Result<Value, RuntimeError> {
        let name = if front { "наСледующемТике" } else { "сразу" };
        let cb = take_callback(args.into_iter().next(), name, span)?;
        let task = Box::new(move |interp: &mut Interpreter, sp: Span| interp.call_function(cb, vec![], sp).map(|_| ()));
        if front {
            self.microtasks.push_front(task);
        } else {
            self.microtasks.push_back(task);
        }
        Ok(Value::Undefined)
    }

    pub(crate) fn drive_event_loop(&mut self, span: Span) -> Result<(), RuntimeError> {
        loop {
            self.drain_microtasks(span)?;
            if self.macrotasks.is_empty() {
                return Ok(());
            }
            let Some(task) = self.macrotasks.pop_next_blocking() else {
                return Ok(());
            };
            (task.task)(self, span)?;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::RefCell;
    use std::rc::Rc;

    #[test]
    fn schedules_fire_in_deadline_order() {
        let mut q = MacrotaskQueue::new();
        let log: Rc<RefCell<Vec<&'static str>>> = Rc::new(RefCell::new(Vec::new()));
        let log_a = Rc::clone(&log);
        let log_b = Rc::clone(&log);
        q.schedule(
            Duration::from_millis(30),
            Box::new(move |_, _| {
                log_a.borrow_mut().push("A");
                Ok(())
            }),
        );
        q.schedule(
            Duration::from_millis(5),
            Box::new(move |_, _| {
                log_b.borrow_mut().push("B");
                Ok(())
            }),
        );
        let mut order: Vec<u64> = Vec::new();
        while let Some(t) = q.pop_next_blocking() {
            order.push(t.id);
            (t.task)(&mut Interpreter::new(), Span { start: 0, end: 0 }).unwrap();
        }
        assert_eq!(*log.borrow(), vec!["B", "A"]);
        assert_eq!(order.len(), 2);
    }

    #[test]
    fn cancel_prevents_fire() {
        let mut q = MacrotaskQueue::new();
        let fired = Rc::new(RefCell::new(false));
        let fired_c = Rc::clone(&fired);
        let id = q.schedule(
            Duration::from_millis(1),
            Box::new(move |_, _| {
                *fired_c.borrow_mut() = true;
                Ok(())
            }),
        );
        q.cancel(id);
        assert!(q.is_empty());
        let popped = q.pop_next_blocking();
        assert!(popped.is_none());
        assert!(!*fired.borrow());
    }

    #[test]
    fn allocate_id_reserves_distinct_ids() {
        let mut q = MacrotaskQueue::new();
        let a = q.allocate_id();
        let b = q.allocate_id();
        assert_ne!(a, b);
    }
}
