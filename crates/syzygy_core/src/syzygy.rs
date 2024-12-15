use std::sync::Arc;

use crate::{
    context::{event::EventContext, Context, FromContext},
    dispatch::{DispatchEffect, Dispatcher},
    event_bus::{EmitEvent, EventBus, Subscribe, Unsubscribe},
    model::{ModelAccess, ModelMut, Models, ModelsBuilder},
    role::{self, Role, RoleGuarded, RoleHolder},
    resource::{ResourceAccess, Resources, ResourcesBuilder},
    spawn::SpawnThread,
};

#[cfg(feature = "async")]
use crate::spawn::SpawnAsync;
#[cfg(feature = "parallel")]
use crate::spawn::SpawnParallel;

#[derive(Debug, Default)]
pub struct SyzygyBuilder {
    models: ModelsBuilder,
    resources: ResourcesBuilder,
    #[cfg(feature = "async")]
    tokio_rt: Option<tokio::runtime::Runtime>,
    #[cfg(feature = "parallel")]
    rayon_pool: Option<rayon::ThreadPool>,
}

impl SyzygyBuilder {
    #[must_use]
    pub fn model<M>(self, model: M) -> Self
    where
        M: RoleGuarded + 'static,
    {
        Self {
            models: self.models.insert(model),
            resources: self.resources,
            #[cfg(feature = "async")]
            tokio_rt: self.tokio_rt,
            #[cfg(feature = "parallel")]
            rayon_pool: self.rayon_pool,
        }
    }

    #[must_use]
    pub fn resource<T>(self, resource: T) -> Self
    where
        T: Clone + Send + Sync + 'static,
    {
        Self {
            models: self.models,
            resources: self.resources.insert(resource),
            #[cfg(feature = "async")]
            tokio_rt: self.tokio_rt,
            #[cfg(feature = "parallel")]
            rayon_pool: self.rayon_pool,
        }
    }

    #[cfg(feature = "async")]
    #[must_use]
    pub fn tokio_rt(self, rt: tokio::runtime::Runtime) -> Self {
        Self {
            models: self.models,
            resources: self.resources,
            tokio_rt: Some(rt),
            #[cfg(feature = "parallel")]
            rayon_pool: self.rayon_pool,
        }
    }

    #[cfg(feature = "parallel")]
    #[must_use]
    pub fn rayon_pool(self, pool: rayon::ThreadPool) -> Self {
        Self {
            models: self.models,
            resources: self.resources,
            #[cfg(feature = "async")]
            tokio_rt: self.tokio_rt,
            rayon_pool: Some(pool),
        }
    }

    #[must_use]
    pub fn build(self) -> Syzygy {
        let models = self.models.build();
        let resources = self.resources.build();

        Syzygy {
            models,
            resources,
            dispatcher: Dispatcher::default(),
            event_bus: EventBus::default(),
            #[cfg(feature = "async")]
            tokio_rt: Arc::new(self.tokio_rt.unwrap()),
            #[cfg(feature = "parallel")]
            rayon_pool: Arc::new(
                self.rayon_pool
                    .unwrap_or_else(|| rayon::ThreadPoolBuilder::new().build().unwrap()),
            ),
        }
    }
}

#[derive(Debug, Clone)]
pub struct Syzygy {
    pub(crate) models: Models,
    pub(super) resources: Resources,
    pub(crate) dispatcher: Dispatcher,
    pub(crate) event_bus: EventBus,
    #[cfg(feature = "async")]
    pub(crate) tokio_rt: Arc<tokio::runtime::Runtime>,
    #[cfg(feature = "parallel")]
    pub(crate) rayon_pool: Arc<rayon::ThreadPool>,
}

impl Syzygy {
    pub(crate) fn handle_effects(&self) {
        while let Some(effect) = self.dispatcher.pop() {
            effect.handle(self.clone());
        }
    }

    pub(crate) fn handle_events(&self) {
        let cx = EventContext::from_context(self.clone());
        while let Some((event_type, event)) = self.event_bus.pop() {
            if let Err(err) = self.event_bus.handle(&cx, &event_type, event) {
                if log::log_enabled!(log::Level::Warn) {
                    log::warn!("{}", err);
                }
            }
        }
    }

    pub fn handle_waiting(&self) {
        self.handle_effects();
        self.handle_events();
    }
}

impl Context for Syzygy {}

// impl HasPermission for Syzygy {
//     type Permission = permission::None;
// }

impl FromContext<Syzygy> for Syzygy {
    fn from_context(cx: Syzygy) -> Self {
        cx
    }
}

impl ModelAccess for Syzygy {
    fn models(&self) -> &Models {
        &self.models
    }
}

impl ModelMut for Syzygy {}

impl ResourceAccess for Syzygy {
    fn resources(&self) -> &Resources {
        &self.resources
    }
}

impl DispatchEffect for Syzygy {
    fn dispatcher(&self) -> &Dispatcher {
        &self.dispatcher
    }
}

impl EmitEvent for Syzygy {
    fn event_bus(&self) -> &EventBus {
        &self.event_bus
    }
}

impl Subscribe for Syzygy {}
impl Unsubscribe for Syzygy {}

impl SpawnThread for Syzygy {}

#[cfg(feature = "async")]
impl SpawnAsync for Syzygy {
    fn tokio_rt(&self) -> &tokio::runtime::Runtime {
        &self.tokio_rt
    }
}

#[cfg(feature = "parallel")]
impl SpawnParallel for Syzygy {
    fn rayon_pool(&self) -> &rayon::ThreadPool {
        &self.rayon_pool
    }
}

impl RoleHolder for Syzygy {
    type Role = role::Root;
}

// // impl<M: Model> EffectBuilder<M> {
// //     #[must_use]
// //     pub fn update<F>(mut self, f: F) -> Self
// //     where
// //         F: FnOnce(&mut M) + Send + Sync + 'static,
// //     {
// //         self.updates.push(Box::new(f));
// //         self
// //     }

// //     #[must_use]
// //     pub fn spawn<F>(mut self, f: F) -> Self
// //     where
// //         F: FnOnce(AppContext<Task, M>) + Send + Sync + 'static,
// //     {
// //         self.tasks.push(Box::new(f));
// //         self
// //     }

// //     #[must_use]
// //     #[inline]
// //     pub fn scope<F>(mut self, f: F) -> Self
// //     where
// //         F: for<'scope> FnOnce(&'scope std::thread::Scope<'scope, '_>, AppContext<Task, M>)
// //             + Send
// //             + Sync
// //             + 'static,
// //     {
// //         self.scoped_tasks.push(Box::new(f));
// //         self
// //     }

// //     #[must_use]
// //     pub fn spawn_blocking<F>(mut self, f: F) -> Self
// //     where
// //         F: FnOnce(AppContext<Task, M>) + Send + Sync + 'static,
// //     {
// //         self.blocking_tasks.push(Box::new(f));
// //         self
// //     }

// //     #[cfg(feature = "async")]
// //     #[must_use]
// //     pub fn spawn_async<F, Fut>(mut self, f: F) -> Self
// //     where
// //         F: FnOnce(AppContext<AsyncTask, M>) -> Fut + Send + Sync + 'static,
// //         Fut: Future<Output = ()> + Send + 'static,
// //     {
// //         self.async_tasks.push(Box::new(move |cx| {
// //             Box::pin(f(cx)) as Pin<Box<dyn Future<Output = ()> + Send>>
// //         }));
// //         self
// //     }

// //     #[cfg(feature = "parallel")]
// //     #[must_use]
// //     pub fn spawn_rayon<F>(mut self, f: F) -> Self
// //     where
// //         F: FnOnce(AppContext<Task, M>) + Send + Sync + 'static,
// //     {
// //         self.rayon_tasks.push(Box::new(f));
// //         self
// //     }

// //     #[cfg(feature = "parallel")]
// //     #[must_use]
// //     pub fn rayon_scope<F>(mut self, f: F) -> Self
// //     where
// //         F: FnOnce(&rayon::Scope<'_>, AppContext<Task, M>) + Send + Sync + 'static,
// //     {
// //         self.rayon_scoped_tasks.push(Box::new(f));
// //         self
// //     }

// //     pub fn dispatch(self) {
// //         let updates = self.updates;
// //         let tasks = self.tasks;

// //         #[cfg(feature = "async")]
// //         let blocking_tasks = self.blocking_tasks;
// //         let scoped_tasks = self.scoped_tasks;
// //         #[cfg(feature = "async")]
// //         let async_tasks = self.async_tasks;
// //         #[cfg(feature = "parallel")]
// //         let parallel_tasks = self.rayon_tasks;
// //         #[cfg(feature = "parallel")]
// //         let parallel_scoped_tasks = self.rayon_scoped_tasks;

// //         self.cx.dispatch(move |cx| {
// //             // Apply model updates first
// //             cx.update(move |m| {
// //                 for update in updates {
// //                     update(m);
// //                 }
// //             });

// //             // Spawn regular tasks
// //             for task in tasks {
// //                 cx.spawn(task);
// //             }

// //             // Execute scoped tasks
// //             for task in scoped_tasks {
// //                 cx.scope(task);
// //             }

// //             // Spawn blocking tasks using tokio
// //             #[cfg(feature = "async")]
// //             for task in blocking_tasks {
// //                 cx.spawn_blocking(task);
// //             }

// //             // Spawn async tasks if tokio enabled
// //             #[cfg(feature = "async")]
// //             for task in async_tasks {
// //                 let task = Box::new(task)
// //                     as Box<
// //                         dyn FnOnce(
// //                                 AppContext<AsyncTask, M>,
// //                             )
// //                                 -> Pin<Box<dyn Future<Output = ()> + Send>>
// //                             + Send
// //                             + Sync,
// //                     >;
// //                 cx.spawn_async(task);
// //             }

// //             // Spawn parallel tasks if rayon enabled
// //             #[cfg(feature = "parallel")]
// //             for task in parallel_tasks {
// //                 cx.spawn_rayon(task);
// //             }

// //             // Execute parallel scoped tasks if rayon enabled
// //             #[cfg(feature = "parallel")]
// //             for task in parallel_scoped_tasks {
// //                 cx.rayon_scope(|s, task_cx| task(s, task_cx));
// //             }
// //         });
// //     }
// // }

pub struct Deferred<F: FnOnce()>(Option<F>);

impl<F: FnOnce()> Deferred<F> {
    /// Drop without running the deferred function.
    pub fn abort(mut self) {
        self.0.take();
    }
}

impl<F: FnOnce()> Drop for Deferred<F> {
    fn drop(&mut self) {
        if let Some(f) = self.0.take() {
            f();
        }
    }
}

/// Run the given function when the returned value is dropped (unless it's cancelled).
#[must_use]
pub fn defer<F: FnOnce()>(f: F) -> Deferred<F> {
    Deferred(Some(f))
}

#[allow(clippy::items_after_statements)]
#[allow(clippy::cast_precision_loss)]
#[cfg(test)]
mod tests {

    use super::*;

    #[derive(Debug)]
    struct TestModel {
        counter: i32,
    }

    impl RoleGuarded for TestModel {
        type Role = role::AnyRole;
    }

    #[derive(Debug)]
    struct SecuredModel {
        counter: i32,
    }

    #[derive(Debug, Clone, Copy, Default)]
    pub struct SomePermission;

    impl Role for SomePermission {}

    impl RoleGuarded for SecuredModel {
        type Role = SomePermission;
    }

    #[test]
    fn test_model() {
        let model = TestModel { counter: 0 };
        let syzygy = SyzygyBuilder::default()
            .model(model)
            .resource("test_resource".to_string())
            .build();

        // Test initial state
        let counter = syzygy.model::<TestModel>().counter;
        assert_eq!(counter, 0);

        // Test model() access
        assert_eq!(syzygy.model::<TestModel>().counter, 0);

        // Test model_mut() modification
        syzygy.model_mut::<TestModel>().counter += 1;
        assert_eq!(syzygy.model::<TestModel>().counter, 1);

        // Test try_model() for existing model
        let model = syzygy.try_model::<TestModel>();
        assert!(model.is_some());
        assert_eq!(model.unwrap().counter, 1);

        // Test try_model_mut() for existing model
        let model_mut = syzygy.try_model_mut::<TestModel>();
        assert!(model_mut.is_some());
        model_mut.unwrap().counter += 1;
        assert_eq!(syzygy.model::<TestModel>().counter, 2);

        // Test update
        syzygy.update(|m: &mut TestModel| {
            m.counter = 42;
        });
        assert_eq!(syzygy.model::<TestModel>().counter, 42);

        // Test query
        let value = syzygy.query(|m: &TestModel| m.counter);
        assert_eq!(value, 42);

        // Test permission
        let secured_model = SecuredModel { counter: 0 };
        let syzygy = SyzygyBuilder::default().model(secured_model).build();

        assert_eq!(syzygy.model::<SecuredModel>().counter, 0);
    }

    #[test]
    fn test_resources() {
        let model = TestModel { counter: 0 };
        let syzygy = SyzygyBuilder::default()
            .model(model)
            .resource("test_str".to_string())
            .resource(42i32)
            .resource(std::f64::consts::PI)
            .resource(vec![1, 2, 3])
            .build();

        // Test accessing existing resources
        assert_eq!(syzygy.resource::<String>(), "test_str");
        assert_eq!(syzygy.resource::<i32>(), 42);
        let pi = syzygy.resource::<f64>();
        assert!((pi - std::f64::consts::PI).abs() < f64::EPSILON);
        assert_eq!(syzygy.resource::<Vec<i32>>(), vec![1, 2, 3]);

        // Test try_resource for existing resources
        assert_eq!(syzygy.try_resource::<String>().unwrap(), "test_str");
        assert_eq!(syzygy.try_resource::<i32>().unwrap(), 42);
        let pi = syzygy.try_resource::<f64>().unwrap();
        assert!((pi - std::f64::consts::PI).abs() < f64::EPSILON);
        assert_eq!(syzygy.try_resource::<Vec<i32>>().unwrap(), vec![1, 2, 3]);

        // Test accessing missing resources
        assert!(syzygy.try_resource::<bool>().is_none());
        assert!(syzygy.try_resource::<char>().is_none());
        assert!(syzygy.try_resource::<usize>().is_none());
        assert!(syzygy.try_resource::<Vec<String>>().is_none());
    }

    #[test]
    fn test_dispatch() {
        let model = TestModel { counter: 0 };
        let syzygy = SyzygyBuilder::default().model(model).build();

        // Test dispatching updates
        syzygy
            .dispatch(|cx: Syzygy| {
                cx.update(|m: &mut TestModel| m.counter += 1);
            })
            .unwrap();

        // Test dispatching multiple updates
        syzygy
            .dispatch(|cx: Syzygy| {
                cx.update(|m: &mut TestModel| m.counter += 1);
                cx.update(|m: &mut TestModel| m.counter += 1);
                cx.update(|m: &mut TestModel| m.counter += 1);
            })
            .unwrap();
        syzygy
            .dispatch(|cx: Syzygy| {
                cx.update(|m: &mut TestModel| m.counter += 1);
            })
            .unwrap();
        syzygy.handle_waiting();
        assert_eq!(syzygy.model::<TestModel>().counter, 5);
    }

    #[test]
    fn test_event_subscribe_unsubscribe() {
        use std::sync::atomic::{AtomicI32, Ordering};
        let model = TestModel { counter: 0 };
        let syzygy = SyzygyBuilder::default().model(model).build();

        #[derive(Debug)]
        struct TestEvent(i32);

        // Setup atomic counter for events
        let counter = Arc::new(AtomicI32::new(0));
        let counter_clone = Arc::clone(&counter);

        // Subscribe handler
        syzygy
            .subscribe::<TestEvent>(Some("test_handler"), move |_cx, event| {
                counter_clone.fetch_add(event.0, Ordering::SeqCst);
            })
            .unwrap();

        // Emit events
        syzygy.emit(TestEvent(1)).unwrap();
        syzygy.emit(TestEvent(2)).unwrap();
        syzygy.handle_waiting();

        assert_eq!(counter.load(Ordering::SeqCst), 3);

        // Unsubscribe handler
        syzygy.unsubscribe("test_handler").unwrap();

        // Emit more events that should not be handled
        syzygy.emit(TestEvent(4)).unwrap();
        syzygy.handle_waiting();

        assert_eq!(counter.load(Ordering::SeqCst), 3);
    }

    #[test]
    fn test_event_multiple_subscribers() {
        use std::sync::atomic::{AtomicI32, Ordering};
        let model = TestModel { counter: 0 };
        let syzygy = SyzygyBuilder::default().model(model).build();

        #[derive(Debug)]
        struct TestEvent(i32);

        // Setup atomic counters for different handlers
        let counter1 = Arc::new(AtomicI32::new(0));
        let counter2 = Arc::new(AtomicI32::new(0));

        let counter1_clone = Arc::clone(&counter1);
        let counter2_clone = Arc::clone(&counter2);

        // Subscribe multiple handlers
        syzygy
            .event_bus
            .subscribe::<TestEvent>(Some("handler1"), move |_cx, event| {
                counter1_clone.fetch_add(event.0, Ordering::SeqCst);
            })
            .unwrap();

        syzygy
            .event_bus
            .subscribe::<TestEvent>(Some("handler2"), move |_cx, event| {
                counter2_clone.fetch_add(event.0 * 2, Ordering::SeqCst);
            })
            .unwrap();

        // Emit event
        syzygy.emit(TestEvent(5)).unwrap();
        syzygy.handle_waiting();

        // Check both handlers processed event
        assert_eq!(counter1.load(Ordering::SeqCst), 5);
        assert_eq!(counter2.load(Ordering::SeqCst), 10);
    }

    #[test]
    fn test_thread_spawn() {
        use std::sync::atomic::{AtomicI32, Ordering};

        let model = TestModel { counter: 0 };
        let syzygy = SyzygyBuilder::default().model(model).build();
        let counter = Arc::new(AtomicI32::new(0));

        // Test spawning a thread that increments counter
        let counter_clone = Arc::clone(&counter);
        let rx = syzygy.spawn(move |_| {
            counter_clone.fetch_add(1, Ordering::SeqCst);
            42 // Return value
        });

        syzygy.handle_waiting();
        // Wait for thread to complete
        let result = rx.recv().unwrap();

        assert_eq!(result, 42);
        assert_eq!(counter.load(Ordering::SeqCst), 1);

        // Test spawning multiple threads
        let threads: Vec<_> = (0..5)
            .map(|_| {
                let counter_clone = Arc::clone(&counter);
                syzygy.spawn(move |_cx| {
                    counter_clone.fetch_add(1, Ordering::SeqCst);
                })
            })
            .collect();

        syzygy.handle_waiting();
        // Wait for all threads
        for rx in threads {
            rx.recv().unwrap();
        }

        assert_eq!(counter.load(Ordering::SeqCst), 6);
    }

    #[cfg(feature = "async")]
    #[test]
    fn test_tokio_spawn() {
        use std::sync::atomic::{AtomicI32, Ordering};

        let model = TestModel { counter: 0 };
        let syzygy = SyzygyBuilder::default()
            .model(model)
            .tokio_rt(tokio::runtime::Runtime::new().unwrap())
            .build();

        let counter = Arc::new(AtomicI32::new(0));

        // Test spawning a task that increments counter
        let counter_clone = Arc::clone(&counter);
        let rx = syzygy.spawn_task(move |_| async move {
            counter_clone.fetch_add(1, Ordering::SeqCst);
            42 // Return value
        });

        syzygy.handle_waiting();
        // Wait for task to complete
        let result = rx.blocking_recv().unwrap();

        assert_eq!(result, 42);
        assert_eq!(counter.load(Ordering::SeqCst), 1);

        // Test spawning multiple tasks
        let tasks: Vec<_> = (0..5)
            .map(|_| {
                let counter_clone = Arc::clone(&counter);
                syzygy.spawn_task(move |_cx| async move {
                    counter_clone.fetch_add(1, Ordering::SeqCst);
                })
            })
            .collect();

        syzygy.handle_waiting();
        // Wait for all tasks
        for rx in tasks {
            rx.blocking_recv().unwrap();
        }

        assert_eq!(counter.load(Ordering::SeqCst), 6);
    }

    #[cfg(feature = "parallel")]
    #[test]
    fn test_parallel_spawn() {
        use std::sync::atomic::{AtomicI32, Ordering};

        let model = TestModel { counter: 0 };
        let syzygy = SyzygyBuilder::default()
            .model(model)
            .rayon_pool(rayon::ThreadPoolBuilder::new().build().unwrap())
            .build();

        let counter = Arc::new(AtomicI32::new(0));

        // Test spawning a parallel task that increments counter
        let counter_clone = Arc::clone(&counter);
        let rx = syzygy.spawn_parallel(move |_| {
            counter_clone.fetch_add(1, Ordering::SeqCst);
            42 // Return value
        });

        syzygy.handle_waiting();
        // Wait for task to complete
        let result = rx.recv().unwrap();

        assert_eq!(result, 42);
        assert_eq!(counter.load(Ordering::SeqCst), 1);

        // Test spawning multiple parallel tasks
        let tasks: Vec<_> = (0..5)
            .map(|_| {
                let counter_clone = Arc::clone(&counter);
                syzygy.spawn_parallel(move |_cx| {
                    counter_clone.fetch_add(1, Ordering::SeqCst);
                })
            })
            .collect();

        syzygy.handle_waiting();
        // Wait for all tasks
        for rx in tasks {
            rx.recv().unwrap();
        }

        assert_eq!(counter.load(Ordering::SeqCst), 6);
    }

    // #[test]
    // #[cfg(not(feature = "async"))]
    // fn test_app_context_query() {
    //     let model = TestModel { counter: 0 };
    //     let cx = GlobalAppContext::builder(model).build();

    //     cx.update(|m| m.counter += 1);
    //     assert_eq!(cx.query(|m| m.counter), 1);
    // }

    // #[test]
    // #[cfg(not(feature = "async"))]
    // fn test_app_context_resource_management() {
    //     let model = TestModel { counter: 0 };
    //     let cx = GlobalAppContext::builder(model).build();

    //     cx.add_resource(42i32).unwrap();
    //     assert_eq!(cx.resource::<i32>().unwrap(), 42);

    //     assert!(cx.add_resource(43i32).is_err());
    // }

    // #[test]
    // #[cfg(not(feature = "async"))]
    // fn test_app_context_graceful_stop() {
    //     let model = TestModel { counter: 0 };
    //     let cx = GlobalAppContext::builder(model).build();

    //     cx.run();
    //     assert!(cx.is_running());
    //     cx.dispatch(|cx: Syzygy<App, TestModel>| {
    //         std::thread::sleep(std::time::Duration::from_millis(100));
    //         cx.update(|m| m.counter += 1)
    //     });

    //     cx.graceful_stop().unwrap();
    //     cx.wait_for_stop().unwrap();
    //     assert!(!cx.is_running());
    //     assert_eq!(cx.query(|m| m.counter), 1);

    //     assert!(cx.graceful_stop().is_err());
    // }

    // #[test]
    // #[cfg(not(feature = "async"))]
    // fn test_app_context_force_stop() {
    //     let model = TestModel { counter: 0 };
    //     let cx = GlobalAppContext::builder(model).build();

    //     cx.run();
    //     assert!(cx.is_running());
    //     cx.dispatch(|cx: Syzygy<App, TestModel>| {
    //         std::thread::sleep(std::time::Duration::from_millis(100));
    //         cx.update(|m| m.counter += 1)
    //     });

    //     cx.force_stop().unwrap();
    //     cx.wait_for_stop().unwrap();
    //     assert!(!cx.is_running());
    //     assert_eq!(cx.query(|m| m.counter), 0);

    //     assert!(cx.force_stop().is_err());
    // }

    // #[test]
    // #[cfg(not(feature = "async"))]
    // fn test_task_context() {
    //     let model = TestModel { counter: 0 };
    //     let cx = GlobalAppContext::builder(model).build();
    //     cx.run();

    //     let handle = cx.spawn(|task_cx| {
    //         task_cx.dispatch(|app_cx: Syzygy<App, TestModel>| {
    //             app_cx.update(|m| {
    //                 m.counter += 1;
    //             });
    //         });
    //         42
    //     });
    //     std::thread::sleep(std::time::Duration::from_millis(1000));

    //     assert_eq!(handle.join().unwrap(), 42);
    //     assert_eq!(cx.query(|m| m.counter), 1);
    // }

    // #[test]
    // #[cfg(feature = "parallel")]
    // fn test_rayon_integration() {
    //     use rayon::prelude::*;

    //     let model = TestModel { counter: 0 };
    //     let cx = GlobalAppContext::builder(model).build();
    //     cx.run();

    //     let counter = Arc::new(std::sync::Mutex::new(0));

    //     (0..100).into_par_iter().for_each(|_| {
    //         let counter_clone = Arc::clone(&counter);
    //         cx.spawn_rayon(move |task_cx| {
    //             let mut lock = counter_clone.lock().unwrap();
    //             *lock += 1;
    //             task_cx.dispatch(|app_cx: Syzygy<App, TestModel>| {
    //                 app_cx.update(|m| m.counter += 1);
    //             });
    //         });
    //     });

    //     // Wait for all tasks to complete
    //     while cx.query(|m| m.counter) < 100 {
    //         std::thread::sleep(std::time::Duration::from_millis(10));
    //     }

    //     assert_eq!(*counter.lock().unwrap(), 100);
    //     assert_eq!(cx.query(|m| m.counter), 100);
    // }

    // #[test]
    // fn test_unsync_threading_safety() {
    //     let model = TestModel { counter: 0 };
    //     let cx = GlobalAppContext::builder(model).build();
    //     cx.run();
    //     // cx.dispatch(|app_cx| {
    //     //     // app_cx.update(|m| {
    //     //     //     m.counter += 1;
    //     //     // });
    //     // });

    //     // let counter = Arc::new(std::sync::atomic::AtomicI32::new(0));
    //     // let threads: Vec<_> = (0..10)
    //     //     .map(|_| {
    //     //         let counter_clone = Arc::clone(&counter);
    //     //         std::thread::spawn(move || {
    //     //             for _ in 0..1000 {
    //     //                 counter_clone.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
    //     //                 cx.dispatch(|app_cx| {
    //     //                     app_cx.update(|m| {
    //     //                         m.counter += 1;
    //     //                     });
    //     //                 });
    //     //             }
    //     //         })
    //     //     })
    //     //     .collect();

    //     // for thread in threads {
    //     //     thread.join().unwrap();
    //     // }

    //     // Wait for dispatched effects
    //     // std::thread::sleep(std::time::Duration::from_secs(1));

    //     // assert_eq!(counter.load(std::sync::atomic::Ordering::SeqCst), 10000);
    //     // assert_eq!(cx.query(|m| m.counter), 10000);
    // }

    // #[test]
    // // #[cfg(not(feature = "async"))]
    // fn test_sync_threading_safety() {
    //     let model = TestModel { counter: 0 };
    //     let cx = GlobalAppContext::builder(model).build();
    //     cx.run();

    //     let counter = Arc::new(std::sync::atomic::AtomicI32::new(0));
    //     let threads: Vec<_> = (0..10)
    //         .map(|_| {
    //             let counter_clone = Arc::clone(&counter);
    //             std::thread::spawn(move || {
    //                 for _ in 0..1000 {
    //                     counter_clone.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
    //                     cx.model_mut().counter += 1;
    //                     cx.dispatch(|cx: Syzygy<App, TestModel>| {
    //                         cx.update(|m| {
    //                             m.counter += 1;
    //                         });
    //                     });
    //                 }
    //             })
    //         })
    //         .collect();

    //     for thread in threads {
    //         thread.join().unwrap();
    //     }
    //     cx.dispatch(|cx: Syzygy<App, TestModel>| cx.graceful_stop().unwrap());
    //     cx.wait_for_stop().unwrap();

    //     // Wait for dispatched effects
    //     std::thread::sleep(std::time::Duration::from_secs(3));

    //     assert_eq!(counter.load(std::sync::atomic::Ordering::SeqCst), 10000);
    //     assert_eq!(cx.query(|m| m.counter), 20000);
    // }

    // #[test]
    // #[cfg(not(feature = "async"))]
    // fn test_scope() {
    //     let model = TestModel { counter: 0 };
    //     let cx = GlobalAppContext::builder(model).build();
    //     cx.run();

    //     let counter = Arc::new(std::sync::atomic::AtomicI32::new(0));

    //     let counter_clone = Arc::clone(&counter);
    //     cx.scope(move |scope, task_cx| {
    //         scope.spawn(move || {
    //             counter_clone.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
    //         });
    //         task_cx.dispatch(|app_cx: Syzygy<App, TestModel>| {
    //             app_cx.update(|m| {
    //                 m.counter += 1;
    //             });
    //         });
    //     });

    //     // Wait for effects to complete
    //     std::thread::sleep(std::time::Duration::from_secs(1));

    //     assert_eq!(counter.load(std::sync::atomic::Ordering::SeqCst), 1);
    //     assert_eq!(cx.query(|m| m.counter), 1);
    // }

    // #[test]
    // fn test_capabilities() {
    //     fn test_read_model<C: CanReadModel>(cx: Syzygy<C, TestModel>) {
    //         assert_eq!(cx.query(|m| m.counter), 0);
    //     }

    //     fn test_modify_model<C: CanModifyModel>(cx: Syzygy<C, TestModel>) {
    //         cx.update(|m| m.counter += 1);
    //         assert_eq!(cx.query(|m| m.counter), 1);
    //     }

    //     fn test_add_resource<C: CanModifyResources>(cx: Syzygy<C, TestModel>) {
    //         cx.add_resource(42i32).unwrap();
    //         assert_eq!(cx.resource::<i32>().unwrap(), 42);
    //     }

    //     fn test_read_resource<C: CanReadResources>(cx: Syzygy<C, TestModel>) {
    //         assert_eq!(cx.resource::<i32>().unwrap(), 42);
    //     }

    //     let model = TestModel { counter: 0 };
    //     let cx = GlobalAppContext::builder(model).build();
    //     cx.run();

    //     test_read_model(cx);
    //     test_modify_model(cx);
    //     test_add_resource(cx);
    //     test_read_resource(cx);
    // }

    // #[ignore]
    // #[tokio::test]
    // #[cfg(feature = "async")]
    // async fn test_tokio_integration() {
    //     let model = TestModel { counter: 0 };
    //     let cx = GlobalAppContext::builder(model)
    //         .handle(tokio::runtime::Handle::current())
    //         .build();
    //     cx.run();

    //     let counter = Arc::new(tokio::sync::Mutex::new(0));

    //     let handles: Vec<_> = (0..100)
    //         .map(|_| {
    //             let counter_clone = Arc::clone(&counter);
    //             cx.spawn_async(move |async_cx| async move {
    //                 let mut lock = counter_clone.lock().await;
    //                 *lock += 1;
    //                 async_cx.dispatch(move |cx: Syzygy<App, TestModel>| {
    //                     cx.update(|m| m.counter += 1);
    //                 });
    //             })
    //         })
    //         .collect();

    //     for handle in handles {
    //         handle.await.unwrap();
    //     }

    //     tokio::time::sleep(std::time::Duration::from_millis(100)).await;

    //     assert_eq!(*counter.lock().await, 100);
    //     assert_eq!(cx.query(|m| m.counter), 100);
    // }

    // #[ignore]
    // #[tokio::test]
    // #[cfg(feature = "async")]
    // async fn test_app_context_async() {
    //     use parking_lot::Mutex;

    //     let model = TestModel { counter: 0 };
    //     let cx = GlobalAppContext::builder(model).build();

    //     let counter = Arc::new(Mutex::new(0));
    //     let counter_clone = Arc::clone(&counter);

    //     cx.dispatch(move |_cx: Syzygy<App, TestModel>| {
    //         let mut lock = counter_clone.lock();
    //         *lock += 1;
    //     });

    //     cx.handle_effects();

    //     assert_eq!(*counter.lock(), 1);
    // }

    // // #[ignore]
    // // #[tokio::test]
    // // #[cfg(feature = "async")]
    // // async fn test_effect_builder() {
    // //     let model = TestModel { counter: 0 };
    // //     let cx = GlobalAppContext::builder(model).build();

    // //     cx.run();

    // //     let shared_counter = Arc::new(std::sync::atomic::AtomicI32::new(0));
    // //     let counter = Arc::clone(&shared_counter);

    // //     let effect = cx
    // //         .effect()
    // //         .update(|m| m.counter += 1)
    // //         .spawn(|task_cx| {
    // //             task_cx.dispatch(|app_cx| {
    // //                 app_cx.update(|m| m.counter += 1);
    // //             });
    // //         })
    // //         .spawn_blocking(|task_cx| {
    // //             task_cx.dispatch(|app_cx| {
    // //                 app_cx.update(|m| m.counter += 1);
    // //             });
    // //         })
    // //         .scope(move |scope, task_cx| {
    // //             for _ in 0..5 {
    // //                 let counter = Arc::clone(&counter);
    // //                 scope.spawn(move || {
    // //                     counter.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
    // //                     task_cx.dispatch(|app_cx| {
    // //                         app_cx.update(|m| m.counter += 1);
    // //                     });
    // //                 });
    // //             }
    // //         })
    // //         .spawn_async(|async_cx| async move {
    // //             async_cx.dispatch(|app_cx| {
    // //                 app_cx.update(|m| m.counter += 1);
    // //             });
    // //         });

    // //     #[cfg(feature = "parallel")]
    // //     let effect = effect
    // //         .spawn_rayon(|task_cx| {
    // //             task_cx.dispatch(|app_cx| {
    // //                 app_cx.update(|m| m.counter += 1);
    // //             });
    // //         })
    // //         .rayon_scope(|_scope, task_cx| {
    // //             task_cx.dispatch(|app_cx| {
    // //                 app_cx.update(|m| m.counter += 1);
    // //             });
    // //         });

    // //     effect.dispatch();

    // //     // Wait for effects to complete
    // //     std::thread::sleep(std::time::Duration::from_secs(1));

    // //     assert_eq!(shared_counter.load(std::sync::atomic::Ordering::SeqCst), 5);

    // //     #[cfg(all(feature = "async", feature = "rayon"))]
    // //     assert_eq!(cx.query(|m| m.counter), 10);

    // //     #[cfg(all(feature = "async", not(feature = "rayon")))]
    // //     assert_eq!(cx.query(|m| m.counter), 8);
    // // }

    // #[test]
    // #[cfg(not(feature = "async"))]
    // fn test_wait_for_stop() {
    //     let model = TestModel { counter: 0 };
    //     let cx = GlobalAppContext::builder(model).build();

    //     cx.run();
    //     assert!(cx.is_running());

    //     let cx_clone = cx;
    //     let handle = std::thread::spawn(move || {
    //         std::thread::sleep(std::time::Duration::from_millis(100));
    //         cx_clone.graceful_stop().unwrap();
    //     });

    //     cx.wait_for_stop().unwrap();
    //     handle.join().unwrap();
    //     assert!(!cx.is_running());
    // }
    #[ignore]
    #[test]
    fn benchmark_dispatch_model_read() {
        use std::time::Instant;

        let model = TestModel { counter: 0 };
        let syzygy = SyzygyBuilder::default().model(model).build();

        const ITERATIONS: usize = 1_000_000;
        const RUNS: usize = 10;

        let mut best_duration = std::time::Duration::from_secs(u64::MAX);

        for _ in 0..RUNS {
            let start = Instant::now();

            for _ in 0..ITERATIONS {
                syzygy
                    .dispatch(|cx: Syzygy| {
                        cx.query(|m: &TestModel| m.counter);
                    })
                    .unwrap();
            }

            let duration = start.elapsed();
            best_duration = best_duration.min(duration);
        }

        let ops_per_sec = ITERATIONS as f64 / best_duration.as_secs_f64();

        println!(
            "Dispatch with model read benchmark:\n\
             {ITERATIONS} iterations in {best_duration:?} (best of {RUNS} runs)\n\
             {ops_per_sec:.2} ops/sec",
        );
    }
    // #[test]
    // fn test_event() {
    //     let model = TestModel { counter: 0 };
    //     let cx = GlobalAppContext::builder(model).build();
    //     cx.run();

    //     #[derive(Debug)]
    //     struct TestEvent {
    //         value: i32,
    //     }

    //     let event_fired = Arc::new(AtomicBool::new(false));
    //     let event_fired_clone = Arc::clone(&event_fired);

    //     cx.on(move |cx, event: &TestEvent| {
    //         assert_eq!(event.value, 42);
    //         cx.update(|m| m.counter = event.value);
    //         event_fired_clone.store(true, std::sync::atomic::Ordering::SeqCst);
    //     });

    //     let event = TestEvent { value: 42 };
    //     cx.dispatch(move |cx: Syzygy<App, TestModel>| {
    //         cx.emit(event).unwrap();
    //     });

    //     std::thread::sleep(std::time::Duration::from_millis(100));
    //     assert!(event_fired.load(std::sync::atomic::Ordering::SeqCst));
    //     assert_eq!(cx.query(|m| m.counter), 42);

    //     cx.graceful_stop().unwrap();
    //     cx.wait_for_stop().unwrap();
    // }

    // #[test]
    // fn test_from_context() {
    //     let model = TestModel { counter: 0 };
    //     let cx = Syzygy::builder(model).build();

    //     #[derive(Debug)]
    //     struct TestContext<M: Model> {
    //         cx: Syzygy<App, M>,
    //     }

    //     impl<M: Model> FromContext<App, M> for TestContext<M> {

    //         fn from_context(cx: Syzygy<App, M>) -> Self {
    //             Self { cx }
    //         }
    //     }

    //     dbg!(cx.context::<TestContext<_>>());
    // }
}
