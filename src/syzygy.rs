use bon::Builder;

use crate::{
    context::Context,
    effects::EffectQueue,
    model::{Model, ModelAccess, ModelModify},
    prelude::ResourceModify,
    resource::{ResourceAccess, Resources},
    spawn::SpawnThread,
};

use crate::effects::{EffectSender, SendEffect};
#[cfg(feature = "parallel")]
use crate::spawn::{RayonPool, SpawnParallel};
use crate::spawn::{SpawnAsync, TokioHandle};

#[derive(Debug, Builder)]
pub struct Syzygy<M: Model> {
    #[builder(field)]
    pub resources: Resources,
    #[builder(field)]
    pub effect_bus: EffectQueue<M>,
    #[builder(into)]
    pub tokio_handle: TokioHandle,
    #[cfg(feature = "parallel")]
    #[builder(into)]
    pub rayon_pool: RayonPool,
    pub model: M,
}

impl<M: Model, S: syzygy_builder::State> SyzygyBuilder<M, S> {
    pub fn resource<T>(mut self, resource: T) -> SyzygyBuilder<M, S>
    where
        T: Clone + Send + Sync + 'static,
    {
        self.resources.insert(resource);
        self
    }
    pub fn effect_hook<H>(mut self, hook: H) -> SyzygyBuilder<M, S>
    where
        H: Fn() + Send + Sync + 'static,
    {
        self.effect_bus.effect_sender.effect_hook = Some(Box::new(hook));
        self
    }
}

impl<M: Model> Syzygy<M> {
    // fn process_middlewares(&mut self, effects: &mut [E]) {
    //     if let Some(middlewares) = &mut self.effect_bus.effect_receiver.middlewares {
    //         for effect in effects {
    //             for middleware in middlewares.iter_mut() {
    //                 middleware.process(effect);
    //             }
    //         }
    //     }
    // }

    pub fn handle_effects(&mut self) {
        while let Some((effects, completion_sender)) = self.effect_bus.effect_receiver.next_effect()
        {
            // self.process_middlewares(&mut effects);

            for effect in effects {
                effect.execute(self);
            }
            if let Some(completion_sender) = completion_sender {
                let _ = completion_sender.send(());
            }
        }
    }
}

impl<M: Model> Context for Syzygy<M> {
    type Model = M;
}

impl<M: Model> ModelAccess for Syzygy<M> {
    #[inline]
    fn model(&self) -> &M {
        &self.model
    }
}

impl<M: Model> ModelModify for Syzygy<M> {
    #[inline]
    fn model_mut(&mut self) -> &mut M {
        &mut self.model
    }
}

impl<M: Model> ResourceAccess for Syzygy<M> {
    #[inline]
    fn resources(&self) -> &Resources {
        &self.resources
    }
}

impl<M: Model> ResourceModify for Syzygy<M> {}

impl<M: Model> SendEffect for Syzygy<M> {
    #[inline]
    fn effect_sender(&self) -> &EffectSender<M> {
        &self.effect_bus.effect_sender
    }
}

impl<M: Model> SpawnThread for Syzygy<M> {}

impl<M: Model> SpawnAsync for Syzygy<M> {
    fn tokio_handle(&self) -> &TokioHandle {
        &self.tokio_handle
    }
}

#[cfg(feature = "parallel")]
impl<M: Model> SpawnParallel<M> for Syzygy<M> {
    fn rayon_pool(&self) -> &RayonPool {
        &self.rayon_pool
    }
}

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
    use std::{fmt, sync::Arc};

    #[derive(Debug, Clone)]
    struct TestModel {
        counter: i32,
    }

    #[derive(Debug, Clone)]
    struct TestResource {
        name: String,
    }

    #[cfg(not(feature = "parallel"))]
    #[test]
    fn test_model() {
        let rt = tokio::runtime::Runtime::new().unwrap();
        let model = TestModel { counter: 0 };
        let mut syzygy: Syzygy<TestModel> = Syzygy::builder()
            .model(model)
            .tokio_handle(rt.handle().clone())
            .build();

        // Test initial state
        let counter = syzygy.model().counter;
        assert_eq!(counter, 0);

        // Test model() access
        assert_eq!(syzygy.model().counter, 0);

        // Test model_mut() modification
        syzygy.model_mut().counter += 1;
        assert_eq!(syzygy.model().counter, 1);

        // Test update
        syzygy.update(|m| {
            m.counter = 42;
        });
        assert_eq!(syzygy.model().counter, 42);

        // Test query
        let value = syzygy.query(|m| m.counter);
        assert_eq!(value, 42);
    }

    #[cfg(not(feature = "parallel"))]
    #[test]
    fn test_resources() {
        let rt = tokio::runtime::Runtime::new().unwrap();
        let model = TestModel { counter: 0 };
        let test_resource = TestResource {
            name: "test_str".to_string(),
        };
        let syzygy: Syzygy<TestModel> = Syzygy::builder()
            .model(model)
            .resource(test_resource)
            .tokio_handle(rt.handle().clone())
            .build();

        // Test accessing existing TestResource
        assert_eq!(syzygy.resource::<TestResource>().name, "test_str");

        // Test try_resource for existing resources
        let test_resource = syzygy.try_resource::<TestResource>();
        assert!(test_resource.is_some());
        assert_eq!(test_resource.unwrap().name, "test_str");
    }

    #[cfg(not(feature = "parallel"))]
    #[test]
    fn test_dispatch() {
        let rt = tokio::runtime::Runtime::new().unwrap();
        let model = TestModel { counter: 0 };
        let mut syzygy: Syzygy<TestModel> = Syzygy::builder()
            .model(model)
            .tokio_handle(rt.handle().clone())
            .effect_hook(|| println!("effect hook"))
            .build();

        // Test dispatching updates
        syzygy.dispatch(|cx| {
            cx.update(|m| m.counter += 1);
        });

        // Test dispatching multiple updates
        syzygy.dispatch_many(vec![
            |cx: &mut Syzygy<TestModel>| cx.update(|m| m.counter += 1),
            |cx: &mut Syzygy<TestModel>| cx.update(|m| m.counter += 1),
            |cx: &mut Syzygy<TestModel>| cx.update(|m| m.counter += 1),
        ]);

        syzygy.dispatch(|cx| {
            cx.update(|m| m.counter += 1);
        });

        syzygy.handle_effects();
        assert_eq!(syzygy.model().counter, 5);
    }

    // #[cfg(not(feature = "parallel"))]
    // #[test]
    // fn test_dynamic_dispatch() {
    //     let rt = tokio::runtime::Runtime::new().unwrap();
    //     let model = TestModel { counter: 0 };
    //     let mut syzygy: Syzygy<TestModel, StaticEffect> = Syzygy::builder()
    //         .model(model)
    //         .tokio_handle(rt.handle().clone())
    //         .build();

    //     // Test dispatching updates
    //     syzygy.send(|cx: &mut Syzygy<TestModel, EffectFn>| {
    //         cx.update(|m| m.counter += 1);
    //     });

    //     // Test dispatching multiple updates
    //     syzygy.send_multiple((0..3).map(|_| {
    //         |cx: &mut Syzygy<TestModel, EffectFn>| {
    //             cx.update(|m| m.counter += 1);
    //         }
    //     }));

    //     syzygy.send(EffectFn(Box::new(|cx| {
    //         cx.update(|m| m.counter += 1);
    //     })));

    //     syzygy.handle_effects();
    //     assert_eq!(syzygy.model().counter, 5);
    // }

    #[cfg(not(feature = "parallel"))]
    #[test]
    fn test_blocking_dispatch() {
        let rt = tokio::runtime::Runtime::new().unwrap();
        let model = TestModel { counter: 0 };
        let mut syzygy: Syzygy<TestModel> = Syzygy::builder()
            .model(model)
            .tokio_handle(rt.handle().clone())
            .build();

        syzygy.spawn_task(async |cx| {
            cx.dispatch_awaitable(|cx| cx.update(|model| model.counter += 1))
                .await;
        });

        for _ in 0..3 {
            syzygy.handle_effects();
            std::thread::sleep(std::time::Duration::from_millis(50));
        }
        assert_eq!(syzygy.model().counter, 1);
    }

    // #[cfg(all(feature = "effects", not(feature = "async"), not(feature = "parallel")))]
    // #[test]
    // fn test_thread_spawn() {
    //     use std::sync::atomic::{AtomicI32, Ordering};

    //     let model = TestModel { counter: 0 };
    //     let mut syzygy = Syzygy::builder().model(model).build();
    //     let counter = Arc::new(AtomicI32::new(0));

    //     // Test spawning a thread that increments counter
    //     let counter_clone = Arc::clone(&counter);
    //     let rx = syzygy.spawn(move |_| {
    //         counter_clone.fetch_add(1, Ordering::SeqCst);
    //         42 // Return value
    //     });

    //     syzygy.handle_effects();
    //     // Wait for thread to complete
    //     let result = rx.recv().unwrap();

    //     assert_eq!(result, 42);
    //     assert_eq!(counter.load(Ordering::SeqCst), 1);

    //     // Test spawning multiple threads
    //     let threads: Vec<_> = (0..5)
    //         .map(|_| {
    //             let counter_clone = Arc::clone(&counter);
    //             syzygy.spawn(move |_| {
    //                 counter_clone.fetch_add(1, Ordering::SeqCst);
    //             })
    //         })
    //         .collect();

    //     syzygy.handle_effects();
    //     // Wait for all threads
    //     for rx in threads {
    //         rx.recv().unwrap();
    //     }

    //     assert_eq!(counter.load(Ordering::SeqCst), 6);
    // }

    // #[cfg(all(feature = "effects", feature = "async", not(feature = "parallel")))]
    // #[test]
    // fn test_tokio_spawn() {
    //     use std::sync::atomic::{AtomicI32, Ordering};

    //     let model = TestModel { counter: 0 };
    //     let resource = TestResource {
    //         name: "test_str".to_string(),
    //     };
    //     let rt = tokio::runtime::Runtime::new().unwrap();
    //     let syzygy = Syzygy::builder()
    //         .model(model)
    //         .tokio_handle(rt.handle().clone())
    //         .build();

    //     let counter = Arc::new(AtomicI32::new(0));

    //     // fn sync_handler<C>(cx: C)
    //     // where
    //     //     C: SpawnThread,
    //     // {
    //     //     println!("async_handler");
    //     //     cx.resource::<TestResource>(); // działa
    //     //     cx.spawn(|cx| {
    //     //         //error
    //     //         cx.resource::<TestResource>();
    //     //     });
    //     //     // cx.spawn_task(|cx| async {
    //     //     //     cx.resource::<TestResource>();
    //     //     // })
    //     // }
    //     // działa
    //     syzygy.spawn(|cx| {
    //         cx.resource::<TestResource>();
    //     });
    //     // nie działa
    //     syzygy.spawn(|cx| {
    //         cx.resource::<TestResource>();
    //     });
    //     // działa
    //     syzygy.spawn(|cx| println!("hello from thread"));

    //     async fn async_handler<C: 'static, M: 'static>(cx: C)
    //     where
    //         C: SpawnAsync<M>,
    //     {
    //         println!("async_handler");
    //         cx.resource::<TestResource>();
    //         // cx.spawn(|cx| {
    //         //     cx.resource::<TestResource>();
    //         // });
    //         // cx.spawn_task(|cx| async {
    //         //     cx.resource::<TestResource>();
    //         // })
    //     }
    //     syzygy.spawn_task(async_handler);

    //     // Test spawning a task that increments counter
    //     let counter_clone = Arc::clone(&counter);
    //     let rx = syzygy.spawn_task(move |cx| async move {
    //         counter_clone.fetch_add(1, Ordering::SeqCst);
    //         42 // Return value
    //     });

    //     syzygy.handle_waiting();
    //     // Wait for task to complete
    //     let result = rx.blocking_recv().unwrap();

    //     assert_eq!(result, 42);
    //     assert_eq!(counter.load(Ordering::SeqCst), 1);

    //     // Test spawning multiple tasks
    //     let tasks: Vec<_> = (0..5)
    //         .map(|_| {
    //             let counter_clone = Arc::clone(&counter);
    //             syzygy.spawn_task(move |cx| async move {
    //                 counter_clone.fetch_add(1, Ordering::SeqCst);
    //             })
    //         })
    //         .collect();

    //     syzygy.handle_waiting();
    //     // Wait for all tasks
    //     for rx in tasks {
    //         rx.blocking_recv().unwrap();
    //     }

    //     assert_eq!(counter.load(Ordering::SeqCst), 6);
    // }

    // #[cfg(all(feature = "parallel", not(feature = "async")))]
    // #[test]
    // fn test_parallel_spawn() {
    //     use std::sync::atomic::{AtomicI32, Ordering};

    //     let model = TestModel { counter: 0 };
    //     let syzygy = Syzygy::builder()
    //         .model(model)
    //         .rayon_pool(rayon::ThreadPoolBuilder::new().build().unwrap())
    //         .build();

    //     let counter = Arc::new(AtomicI32::new(0));

    //     // Test spawning a parallel task that increments counter
    //     let counter_clone = Arc::clone(&counter);

    //     let rx = syzygy.spawn_parallel(move |cx| {
    //         counter_clone.fetch_add(1, Ordering::SeqCst);
    //         42 // Return value
    //     });

    //     syzygy.handle_waiting();
    //     // Wait for task to complete
    //     let result = rx.recv().unwrap();

    //     assert_eq!(result, 42);
    //     assert_eq!(counter.load(Ordering::SeqCst), 1);

    //     // Test spawning multiple parallel tasks
    //     let tasks: Vec<_> = (0..5)
    //         .map(|_| {
    //             let counter_clone = Arc::clone(&counter);
    //             syzygy.spawn_parallel(move |_cx| {
    //                 counter_clone.fetch_add(1, Ordering::SeqCst);
    //             })
    //         })
    //         .collect();

    //     syzygy.handle_waiting();
    //     // Wait for all tasks
    //     for rx in tasks {
    //         rx.recv().unwrap();
    //     }

    //     assert_eq!(counter.load(Ordering::SeqCst), 6);
    // }

    // // #[test]
    // // #[cfg(not(feature = "async"))]
    // // fn test_app_context_query() {
    // //     let model = TestModel { counter: 0 };
    // //     let cx = GlobalAppContext::builder(model).build();

    // //     cx.update(|m| m.counter += 1);
    // //     assert_eq!(cx.query(|m| m.counter), 1);
    // // }

    // // #[test]
    // // #[cfg(not(feature = "async"))]
    // // fn test_app_context_resource_management() {
    // //     let model = TestModel { counter: 0 };
    // //     let cx = GlobalAppContext::builder(model).build();

    // //     cx.add_resource(42i32).unwrap();
    // //     assert_eq!(cx.resource::<i32>().unwrap(), 42);

    // //     assert!(cx.add_resource(43i32).is_err());
    // // }

    // // #[test]
    // // #[cfg(not(feature = "async"))]
    // // fn test_app_context_graceful_stop() {
    // //     let model = TestModel { counter: 0 };
    // //     let cx = GlobalAppContext::builder(model).build();

    // //     cx.run();
    // //     assert!(cx.is_running());
    // //     cx.dispatch(|cx: Syzygy<App, TestModel>| {
    // //         std::thread::sleep(std::time::Duration::from_millis(100));
    // //         cx.update(|m| m.counter += 1)
    // //     });

    // //     cx.graceful_stop().unwrap();
    // //     cx.wait_for_stop().unwrap();
    // //     assert!(!cx.is_running());
    // //     assert_eq!(cx.query(|m| m.counter), 1);

    // //     assert!(cx.graceful_stop().is_err());
    // // }

    // // #[test]
    // // #[cfg(not(feature = "async"))]
    // // fn test_app_context_force_stop() {
    // //     let model = TestModel { counter: 0 };
    // //     let cx = GlobalAppContext::builder(model).build();

    // //     cx.run();
    // //     assert!(cx.is_running());
    // //     cx.dispatch(|cx: Syzygy<App, TestModel>| {
    // //         std::thread::sleep(std::time::Duration::from_millis(100));
    // //         cx.update(|m| m.counter += 1)
    // //     });

    // //     cx.force_stop().unwrap();
    // //     cx.wait_for_stop().unwrap();
    // //     assert!(!cx.is_running());
    // //     assert_eq!(cx.query(|m| m.counter), 0);

    // //     assert!(cx.force_stop().is_err());
    // // }

    // // #[test]
    // // #[cfg(not(feature = "async"))]
    // // fn test_task_context() {
    // //     let model = TestModel { counter: 0 };
    // //     let cx = GlobalAppContext::builder(model).build();
    // //     cx.run();

    // //     let handle = cx.spawn(|task_cx| {
    // //         task_cx.dispatch(|app_cx: Syzygy<App, TestModel>| {
    // //             app_cx.update(|m| {
    // //                 m.counter += 1;
    // //             });
    // //         });
    // //         42
    // //     });
    // //     std::thread::sleep(std::time::Duration::from_millis(1000));

    // //     assert_eq!(handle.join().unwrap(), 42);
    // //     assert_eq!(cx.query(|m| m.counter), 1);
    // // }

    // // #[test]
    // // #[cfg(feature = "parallel")]
    // // fn test_rayon_integration() {
    // //     use rayon::prelude::*;

    // //     let model = TestModel { counter: 0 };
    // //     let cx = GlobalAppContext::builder(model).build();
    // //     cx.run();

    // //     let counter = Arc::new(std::sync::Mutex::new(0));

    // //     (0..100).into_par_iter().for_each(|_| {
    // //         let counter_clone = Arc::clone(&counter);
    // //         cx.spawn_rayon(move |task_cx| {
    // //             let mut lock = counter_clone.lock().unwrap();
    // //             *lock += 1;
    // //             task_cx.dispatch(|app_cx: Syzygy<App, TestModel>| {
    // //                 app_cx.update(|m| m.counter += 1);
    // //             });
    // //         });
    // //     });

    // //     // Wait for all tasks to complete
    // //     while cx.query(|m| m.counter) < 100 {
    // //         std::thread::sleep(std::time::Duration::from_millis(10));
    // //     }

    // //     assert_eq!(*counter.lock().unwrap(), 100);
    // //     assert_eq!(cx.query(|m| m.counter), 100);
    // // }

    // // #[test]
    // // fn test_unsync_threading_safety() {
    // //     let model = TestModel { counter: 0 };
    // //     let cx = GlobalAppContext::builder(model).build();
    // //     cx.run();
    // //     // cx.dispatch(|app_cx| {
    // //     //     // app_cx.update(|m| {
    // //     //     //     m.counter += 1;
    // //     //     // });
    // //     // });

    // //     // let counter = Arc::new(std::sync::atomic::AtomicI32::new(0));
    // //     // let threads: Vec<_> = (0..10)
    // //     //     .map(|_| {
    // //     //         let counter_clone = Arc::clone(&counter);
    // //     //         std::thread::spawn(move || {
    // //     //             for _ in 0..1000 {
    // //     //                 counter_clone.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
    // //     //                 cx.dispatch(|app_cx| {
    // //     //                     app_cx.update(|m| {
    // //     //                         m.counter += 1;
    // //     //                     });
    // //     //                 });
    // //     //             }
    // //     //         })
    // //     //     })
    // //     //     .collect();

    // //     // for thread in threads {
    // //     //     thread.join().unwrap();
    // //     // }

    // //     // Wait for dispatched effects
    // //     // std::thread::sleep(std::time::Duration::from_secs(1));

    // //     // assert_eq!(counter.load(std::sync::atomic::Ordering::SeqCst), 10000);
    // //     // assert_eq!(cx.query(|m| m.counter), 10000);
    // // }

    // // #[test]
    // // // #[cfg(not(feature = "async"))]
    // // fn test_sync_threading_safety() {
    // //     let model = TestModel { counter: 0 };
    // //     let cx = GlobalAppContext::builder(model).build();
    // //     cx.run();

    // //     let counter = Arc::new(std::sync::atomic::AtomicI32::new(0));
    // //     let threads: Vec<_> = (0..10)
    // //         .map(|_| {
    // //             let counter_clone = Arc::clone(&counter);
    // //             std::thread::spawn(move || {
    // //                 for _ in 0..1000 {
    // //                     counter_clone.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
    // //                     cx.model_mut().counter += 1;
    // //                     cx.dispatch(|cx: Syzygy<App, TestModel>| {
    // //                         cx.update(|m| {
    // //                             m.counter += 1;
    // //                         });
    // //                     });
    // //                 }
    // //             })
    // //         })
    // //         .collect();

    // //     for thread in threads {
    // //         thread.join().unwrap();
    // //     }
    // //     cx.dispatch(|cx: Syzygy<App, TestModel>| cx.graceful_stop().unwrap());
    // //     cx.wait_for_stop().unwrap();

    // //     // Wait for dispatched effects
    // //     std::thread::sleep(std::time::Duration::from_secs(3));

    // //     assert_eq!(counter.load(std::sync::atomic::Ordering::SeqCst), 10000);
    // //     assert_eq!(cx.query(|m| m.counter), 20000);
    // // }

    // // #[test]
    // // #[cfg(not(feature = "async"))]
    // // fn test_scope() {
    // //     let model = TestModel { counter: 0 };
    // //     let cx = GlobalAppContext::builder(model).build();
    // //     cx.run();

    // //     let counter = Arc::new(std::sync::atomic::AtomicI32::new(0));

    // //     let counter_clone = Arc::clone(&counter);
    // //     cx.scope(move |scope, task_cx| {
    // //         scope.spawn(move || {
    // //             counter_clone.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
    // //         });
    // //         task_cx.dispatch(|app_cx: Syzygy<App, TestModel>| {
    // //             app_cx.update(|m| {
    // //                 m.counter += 1;
    // //             });
    // //         });
    // //     });

    // //     // Wait for effects to complete
    // //     std::thread::sleep(std::time::Duration::from_secs(1));

    // //     assert_eq!(counter.load(std::sync::atomic::Ordering::SeqCst), 1);
    // //     assert_eq!(cx.query(|m| m.counter), 1);
    // // }

    // // #[test]
    // // fn test_capabilities() {
    // //     fn test_read_model<C: CanReadModel>(cx: Syzygy<C, TestModel>) {
    // //         assert_eq!(cx.query(|m| m.counter), 0);
    // //     }

    // //     fn test_modify_model<C: CanModifyModel>(cx: Syzygy<C, TestModel>) {
    // //         cx.update(|m| m.counter += 1);
    // //         assert_eq!(cx.query(|m| m.counter), 1);
    // //     }

    // //     fn test_add_resource<C: CanModifyResources>(cx: Syzygy<C, TestModel>) {
    // //         cx.add_resource(42i32).unwrap();
    // //         assert_eq!(cx.resource::<i32>().unwrap(), 42);
    // //     }

    // //     fn test_read_resource<C: CanReadResources>(cx: Syzygy<C, TestModel>) {
    // //         assert_eq!(cx.resource::<i32>().unwrap(), 42);
    // //     }

    // //     let model = TestModel { counter: 0 };
    // //     let cx = GlobalAppContext::builder(model).build();
    // //     cx.run();

    // //     test_read_model(cx);
    // //     test_modify_model(cx);
    // //     test_add_resource(cx);
    // //     test_read_resource(cx);
    // // }

    // // #[ignore]
    // // #[tokio::test]
    // // #[cfg(feature = "async")]
    // // async fn test_tokio_integration() {
    // //     let model = TestModel { counter: 0 };
    // //     let cx = GlobalAppContext::builder(model)
    // //         .handle(tokio::runtime::Handle::current())
    // //         .build();
    // //     cx.run();

    // //     let counter = Arc::new(tokio::sync::Mutex::new(0));

    // //     let handles: Vec<_> = (0..100)
    // //         .map(|_| {
    // //             let counter_clone = Arc::clone(&counter);
    // //             cx.spawn_async(move |async_cx| async move {
    // //                 let mut lock = counter_clone.lock().await;
    // //                 *lock += 1;
    // //                 async_cx.dispatch(move |cx: Syzygy<App, TestModel>| {
    // //                     cx.update(|m| m.counter += 1);
    // //                 });
    // //             })
    // //         })
    // //         .collect();

    // //     for handle in handles {
    // //         handle.await.unwrap();
    // //     }

    // //     tokio::time::sleep(std::time::Duration::from_millis(100)).await;

    // //     assert_eq!(*counter.lock().await, 100);
    // //     assert_eq!(cx.query(|m| m.counter), 100);
    // // }

    // // #[ignore]
    // // #[tokio::test]
    // // #[cfg(feature = "async")]
    // // async fn test_app_context_async() {
    // //     use parking_lot::Mutex;

    // //     let model = TestModel { counter: 0 };
    // //     let cx = GlobalAppContext::builder(model).build();

    // //     let counter = Arc::new(Mutex::new(0));
    // //     let counter_clone = Arc::clone(&counter);

    // //     cx.dispatch(move |_cx: Syzygy<App, TestModel>| {
    // //         let mut lock = counter_clone.lock();
    // //         *lock += 1;
    // //     });

    // //     cx.handle_effects();

    // //     assert_eq!(*counter.lock(), 1);
    // // }

    // // // #[ignore]
    // // // #[tokio::test]
    // // // #[cfg(feature = "async")]
    // // // async fn test_effect_builder() {
    // // //     let model = TestModel { counter: 0 };
    // // //     let cx = GlobalAppContext::builder(model).build();

    // // //     cx.run();

    // // //     let shared_counter = Arc::new(std::sync::atomic::AtomicI32::new(0));
    // // //     let counter = Arc::clone(&shared_counter);

    // // //     let effect = cx
    // // //         .effect()
    // // //         .update(|m| m.counter += 1)
    // // //         .spawn(|task_cx| {
    // // //             task_cx.dispatch(|app_cx| {
    // // //                 app_cx.update(|m| m.counter += 1);
    // // //             });
    // // //         })
    // // //         .spawn_blocking(|task_cx| {
    // // //             task_cx.dispatch(|app_cx| {
    // // //                 app_cx.update(|m| m.counter += 1);
    // // //             });
    // // //         })
    // // //         .scope(move |scope, task_cx| {
    // // //             for _ in 0..5 {
    // // //                 let counter = Arc::clone(&counter);
    // // //                 scope.spawn(move || {
    // // //                     counter.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
    // // //                     task_cx.dispatch(|app_cx| {
    // // //                         app_cx.update(|m| m.counter += 1);
    // // //                     });
    // // //                 });
    // // //             }
    // // //         })
    // // //         .spawn_async(|async_cx| async move {
    // // //             async_cx.dispatch(|app_cx| {
    // // //                 app_cx.update(|m| m.counter += 1);
    // // //             });
    // // //         });

    // // //     #[cfg(feature = "parallel")]
    // // //     let effect = effect
    // // //         .spawn_rayon(|task_cx| {
    // // //             task_cx.dispatch(|app_cx| {
    // // //                 app_cx.update(|m| m.counter += 1);
    // // //             });
    // // //         })
    // // //         .rayon_scope(|_scope, task_cx| {
    // // //             task_cx.dispatch(|app_cx| {
    // // //                 app_cx.update(|m| m.counter += 1);
    // // //             });
    // // //         });

    // // //     effect.dispatch();

    // // //     // Wait for effects to complete
    // // //     std::thread::sleep(std::time::Duration::from_secs(1));

    // // //     assert_eq!(shared_counter.load(std::sync::atomic::Ordering::SeqCst), 5);

    // // //     #[cfg(all(feature = "async", feature = "rayon"))]
    // // //     assert_eq!(cx.query(|m| m.counter), 10);

    // // //     #[cfg(all(feature = "async", not(feature = "rayon")))]
    // // //     assert_eq!(cx.query(|m| m.counter), 8);
    // // // }

    // // #[test]
    // // #[cfg(not(feature = "async"))]
    // // fn test_wait_for_stop() {
    // //     let model = TestModel { counter: 0 };
    // //     let cx = GlobalAppContext::builder(model).build();

    // //     cx.run();
    // //     assert!(cx.is_running());

    // //     let cx_clone = cx;
    // //     let handle = std::thread::spawn(move || {
    // //         std::thread::sleep(std::time::Duration::from_millis(100));
    // //         cx_clone.graceful_stop().unwrap();
    // //     });

    // //     cx.wait_for_stop().unwrap();
    // //     handle.join().unwrap();
    // //     assert!(!cx.is_running());
    // // }
    // #[ignore]
    #[cfg(not(feature = "parallel"))]
    #[test]
    fn test_dispatch_performance() {
        use std::time::Instant;

        let rt = tokio::runtime::Runtime::new().unwrap();
        let model = TestModel { counter: 0 };
        let mut syzygy: Syzygy<TestModel> = Syzygy::builder()
            .model(model.clone())
            .tokio_handle(rt.handle().clone())
            .build();

        const ITERATIONS: usize = 1_000_000;
        const RUNS: usize = 10;

        // Test dispatch
        let mut best = std::time::Duration::from_secs(u64::MAX);
        for _ in 0..RUNS {
            let start = Instant::now();
            for _ in 0..ITERATIONS {
                syzygy.dispatch(|cx| {});
            }
            syzygy.handle_effects();
            let duration = start.elapsed();
            best = best.min(duration);
        }
        let ops = ITERATIONS as f64 / best.as_secs_f64();

        println!("Dynamic dispatch: {ITERATIONS} iterations in {best:?} ({ops:.2} ops/sec)");
    }
    #[cfg(all(not(feature = "async"), not(feature = "parallel")))]
    #[test]
    fn benchmark_direct_model_update() {
        use std::time::Instant;

        let rt = tokio::runtime::Runtime::new().unwrap();
        let model = TestModel { counter: 0 };
        let mut syzygy: Syzygy<TestModel> = Syzygy::builder()
            .model(model)
            .tokio_handle(rt.handle().clone())
            .build();

        const ITERATIONS: usize = 1_000_000;
        const RUNS: usize = 10;

        let mut best_duration = std::time::Duration::from_secs(u64::MAX);

        for _ in 0..RUNS {
            let start = Instant::now();

            for _ in 0..ITERATIONS {
                syzygy.update(|m| m.counter += 1);
            }

            let duration = start.elapsed();
            best_duration = best_duration.min(duration);
        }

        let ops_per_sec = ITERATIONS as f64 / best_duration.as_secs_f64();

        println!(
            "Direct model update benchmark:\n\
             {ITERATIONS} iterations in {best_duration:?} (best of {RUNS} runs)\n\
             {ops_per_sec:.2} ops/sec",
        );
    }
}
