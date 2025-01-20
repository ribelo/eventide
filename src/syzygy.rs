use bon::Builder;

use crate::{
    context::Context,
    dispatch::{DispatchEffect, EffectsBus, EffectsTx},
    model::{Model, ModelAccess, ModelModify, ModelSnapshotCreate},
    resource::{ResourceAccess, ResourceModify, Resources},
};

#[derive(Debug, Builder)]
pub struct Syzygy<M: Model> {
    #[builder(field)]
    pub resources: Resources,
    #[builder(field)]
    pub effects_bus: EffectsBus<M>,
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
}

impl<M: Model> Syzygy<M> {
    pub fn handle_effects(&mut self) {
        while let Ok(effect) = self.effects_bus.rx.try_recv() {
            (effect)(self);
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

impl<M: Model> ModelSnapshotCreate for Syzygy<M> {
    #[inline]
    fn create_snapshot(&self) -> <<Self as Context>::Model as Model>::Snapshot {
        self.model.to_snapshot()
    }
}

impl<M: Model> ResourceAccess for Syzygy<M> {
    #[inline]
    fn resources(&self) -> &Resources {
        &self.resources
    }
}

impl<M: Model> ResourceModify for Syzygy<M> {}

impl<M: Model> DispatchEffect for Syzygy<M> {
    #[inline]
    fn effects_tx(&self) -> &EffectsTx<M> {
        &self.effects_bus.tx
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

    #[derive(Debug, Clone)]
    struct TestModel {
        counter: i32,
    }

    impl Model for TestModel {
        type Snapshot = Self;
        fn to_snapshot(&self) -> Self::Snapshot {
            self.clone()
        }
    }

    #[derive(Debug, Clone)]
    struct TestResource {
        name: String,
    }

    fn increment(syzygy: &mut Syzygy<TestModel>) {
        syzygy.model_mut().counter += 1;
    }

    #[cfg(not(feature = "parallel"))]
    #[tokio::test]
    async fn test_model() {
        let model = TestModel { counter: 0 };
        let mut syzygy = Syzygy::builder().model(model).build();

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
    #[tokio::test]
    async fn test_resources() {
        let model = TestModel { counter: 0 };
        let test_resource = TestResource {
            name: "test_str".to_string(),
        };
        let syzygy = Syzygy::builder()
            .model(model)
            .resource(test_resource)
            .build();

        // Test accessing existing TestResource
        assert_eq!(syzygy.resource::<TestResource>().name, "test_str");

        // Test try_resource for existing resources
        let test_resource = syzygy.try_resource::<TestResource>();
        assert!(test_resource.is_some());
        assert_eq!(test_resource.unwrap().name, "test_str");
    }

    #[cfg(not(feature = "parallel"))]
    #[tokio::test]
    async fn test_async_dispatch() {
        let model = TestModel { counter: 0 };
        let mut syzygy: Syzygy<TestModel> = Syzygy::builder().model(model).build();

        // Dispatch the effect multiple times
        for _ in 0..5 {
            syzygy.dispatch(|cx: &mut Syzygy<TestModel>| increment(cx));
        }

        syzygy.handle_effects();

        assert_eq!(syzygy.model().counter, 5);
    }
    #[cfg(not(feature = "parallel"))]
    #[tokio::test]
    async fn test_sync_dispatch() {
        let model = TestModel { counter: 0 };
        let mut syzygy: Syzygy<TestModel> = Syzygy::builder().model(model).build();

        let rx = syzygy.dispatch_sync(increment);

        syzygy.handle_effects();
        rx.await.unwrap();

        assert_eq!(syzygy.model().counter, 1);
    }
    #[cfg(not(feature = "parallel"))]
    #[tokio::test(flavor = "multi_thread")]
    async fn test_thread_task() {
        let model = TestModel { counter: 0 };
        let mut cx: Syzygy<TestModel> = Syzygy::builder().model(model).build();

        cx.spawn(|cx| {
            println!("hello from thread task 1");
            cx.dispatch(increment);
        });

        cx.spawn(|cx| {
            println!("hello from thread task 2");
            cx.dispatch(increment);
        });

        cx.handle_effects();
        std::thread::sleep(std::time::Duration::from_millis(100));
        cx.handle_effects();
        std::thread::sleep(std::time::Duration::from_millis(100));
        cx.handle_effects();
        std::thread::sleep(std::time::Duration::from_millis(100));
        cx.handle_effects();

        assert_eq!(cx.model().counter, 2);
    }
    #[cfg(not(feature = "parallel"))]
    #[tokio::test(flavor = "multi_thread")]
    async fn test_async_task() {
        let model = TestModel { counter: 0 };
        let mut syzygy: Syzygy<TestModel> = Syzygy::builder().model(model).build();

        // First async task
        syzygy.task(|cx| async move {
            println!("hello from thread task 1");
            cx.dispatch(increment);
        });

        // Second async task
        syzygy.task(|cx| async move {
            println!("hello from thread task 2");
            cx.dispatch(increment);
        });

        // Handle effects with some delay to allow tasks to complete
        syzygy.handle_effects();
        std::thread::sleep(std::time::Duration::from_millis(100));
        syzygy.handle_effects();
        std::thread::sleep(std::time::Duration::from_millis(100));
        syzygy.handle_effects();
        std::thread::sleep(std::time::Duration::from_millis(100));
        syzygy.handle_effects();

        assert_eq!(syzygy.model().counter, 2);
    }

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
    #[cfg(not(feature = "parallel"))]
    #[tokio::test]
    async fn test_increment_dispatch() {
        let model = TestModel { counter: 0 };
        let mut syzygy = Syzygy::builder().model(model).build();

        const ITERATIONS: usize = 1_000_000;

        for _ in 0..ITERATIONS {
            syzygy.dispatch(increment);
        }
        syzygy.handle_effects();

        assert_eq!(syzygy.model().counter, ITERATIONS as i32);
    }
    // #[ignore]
    #[cfg(not(feature = "parallel"))]
    #[tokio::test]
    async fn test_dispatch_performance() {
        use std::time::Instant;

        let model = TestModel { counter: 0 };
        let mut syzygy = Syzygy::builder().model(model.clone()).build();

        const ITERATIONS: usize = 1_000_000;
        const RUNS: usize = 10;

        // Test one chain of empty effects
        let mut best_empty = std::time::Duration::from_secs(u64::MAX);
        for _ in 0..RUNS {
            let start = Instant::now();
            for _ in 0..ITERATIONS {
                syzygy.dispatch(|_: &mut Syzygy<TestModel>| {});
            }
            syzygy.handle_effects();
            let duration = start.elapsed();
            best_empty = best_empty.min(duration);
        }
        let ops_empty = ITERATIONS as f64 / best_empty.as_secs_f64();

        // Test multiple small chains of empty effects
        let mut best_chain = std::time::Duration::from_secs(u64::MAX);
        for _ in 0..RUNS {
            let start = Instant::now();
            for _ in 0..ITERATIONS {
                syzygy.dispatch(|_: &mut Syzygy<TestModel>| {});
                syzygy.dispatch(|_: &mut Syzygy<TestModel>| {});
                syzygy.dispatch(|_: &mut Syzygy<TestModel>| {});
                syzygy.dispatch(|_: &mut Syzygy<TestModel>| {});
                syzygy.handle_effects();
            }
            let duration = start.elapsed();
            best_chain = best_chain.min(duration);
        }
        let ops_chain = ITERATIONS as f64 / best_chain.as_secs_f64();

        // Test model mutation effects
        let mut best_mutation = std::time::Duration::from_secs(u64::MAX);
        for _ in 0..RUNS {
            let start = Instant::now();
            for _ in 0..ITERATIONS {
                syzygy.dispatch(|cx: &mut Syzygy<TestModel>| {
                    cx.model_mut().counter += 1;
                });
            }
            syzygy.handle_effects();
            let duration = start.elapsed();
            best_mutation = best_mutation.min(duration);
        }
        let ops_mutation = ITERATIONS as f64 / best_mutation.as_secs_f64();

        // Test chained effects
        let mut best_chained = std::time::Duration::from_secs(u64::MAX);
        for _ in 0..RUNS {
            let start = Instant::now();
            for _ in 0..ITERATIONS {
                syzygy.dispatch(|cx: &mut Syzygy<TestModel>| {
                    cx.dispatch(|cx| cx.model_mut().counter += 1);
                    cx.dispatch(|cx| cx.model_mut().counter += 1);
                    cx.dispatch(|cx| cx.model_mut().counter += 1);
                    cx.dispatch(|cx| cx.model_mut().counter += 1);
                    cx.dispatch(|cx| cx.model_mut().counter += 1);
                    cx.dispatch(|cx| cx.model_mut().counter += 1);
                    cx.dispatch(|cx| cx.model_mut().counter += 1);
                    cx.dispatch(|cx| cx.model_mut().counter += 1);
                });
            }
            syzygy.handle_effects();
            let duration = start.elapsed();
            best_chained = best_chained.min(duration);
        }
        let ops_chained = ITERATIONS as f64 / best_chained.as_secs_f64();

        // Real scenario
        let mut best_real = std::time::Duration::from_secs(u64::MAX);
        for _ in 0..RUNS {
            let start = Instant::now();
            for _ in 0..ITERATIONS {
                syzygy.dispatch(|cx: &mut Syzygy<TestModel>| {
                    // Simulate a more complex real-world scenario with
                    // multiple branching effects and state updates

                    // Update some base state
                    cx.model_mut().counter += 1;

                    // Chain some conditional effects
                    if cx.model().counter % 2 == 0 {
                        cx.model_mut().counter += 2;
                        cx.dispatch(|cx| {
                            cx.model_mut().counter *= 2;
                        });
                    } else {
                        cx.model_mut().counter -= 1;
                        cx.dispatch(|cx| {
                            cx.model_mut().counter += 5;
                        });
                    }

                    // Another conditional branch
                    cx.dispatch(|cx| {
                        if cx.model().counter > 100 {
                            cx.model_mut().counter = 0;
                        }
                    });

                    // Simulate some expensive computation
                    cx.dispatch(|cx| {
                        let mut total = 0;
                        for i in 0..cx.model().counter {
                            total += i;
                        }
                        cx.model_mut().counter = total;
                    });
                });
            }
            syzygy.handle_effects();
            let duration = start.elapsed();
            best_real = best_real.min(duration);
        }
        let ops_real = ITERATIONS as f64 / best_real.as_secs_f64();

        println!("Performance Results:");
        println!("-------------------");
        println!("Empty effects:");
        println!("  Iterations: {ITERATIONS}");
        println!("  Time: {best_empty:?}");
        println!("  Speed: {ops_empty:.2} ops/sec");
        println!();
        println!("Mutation effects:");
        println!("  Iterations: {ITERATIONS}");
        println!("  Time: {best_mutation:?}");
        println!("  Speed: {ops_mutation:.2} ops/sec");
        println!();
        println!("Big chain of empty effects:");
        println!("  Iterations: {ITERATIONS}");
        println!("  Time: {best_chained:?}");
        println!("  Speed: {ops_chained:.2} ops/sec");
        println!();
        println!("Small chains of empty effects:");
        println!("  Iterations: {ITERATIONS}");
        println!("  Time: {best_chain:?}");
        println!("  Speed: {ops_chain:.2} ops/sec");
        println!();
        println!("Real scenario:");
        println!("  Iterations: {ITERATIONS}");
        println!("  Time: {best_real:?}");
        println!("  Speed: {ops_real:.2} ops/sec");
        println!();
    }
    // #[cfg(not(feature = "parallel"))]
    // #[tokio::test]
    // async fn test_task_performance() {
    //     use crate::dispatch::AsyncTask;
    //     use std::time::Instant;

    //     let model = TestModel { counter: 0 };
    //     let mut syzygy = Syzygy::builder().model(model.clone()).build();

    //     const ITERATIONS: usize = 1_000_000;
    //     const RUNS: usize = 10;

    //     let mut best_dispatch = std::time::Duration::from_secs(u64::MAX);
    //     for _ in 0..RUNS {
    //         let start = Instant::now();
    //         for _ in 0..ITERATIONS {
    //             let effects = Effects::new()
    //                 .task(async |_| ())
    //                 .perform(|_| Effects::from(increment));
    //             syzygy.dispatch(effects);
    //         }
    //         syzygy.handle_effects();
    //         let duration = start.elapsed();
    //         best_dispatch = best_dispatch.min(duration);
    //     }
    //     let ops_dispatch = ITERATIONS as f64 / best_dispatch.as_secs_f64();

    //     println!(
    //         "Task dispatch: {ITERATIONS} iterations in {best_dispatch:?} ({ops_dispatch:.2} ops/sec)"
    //     );
    // }
    #[cfg(all(not(feature = "async"), not(feature = "parallel")))]
    #[tokio::test]
    async fn benchmark_direct_model_update() {
        use std::time::Instant;

        let model = TestModel { counter: 0 };
        let mut syzygy: Syzygy<TestModel> = Syzygy::builder().model(model).build();

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
