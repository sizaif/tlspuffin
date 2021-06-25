use core::time::Duration;
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};

use itertools::Itertools;
use libafl::bolts::shmem::{ShMemProvider, StdShMemProvider};
use libafl::corpus::LenTimeMinimizerCorpusScheduler;
use libafl::{
    bolts::{
        current_nanos,
        rands::StdRand,
        tuples::{tuple_list, Merge},
    },
    corpus::{
        Corpus, InMemoryCorpus, IndexesLenTimeMinimizerCorpusScheduler, OnDiskCorpus,
        QueueCorpusScheduler, RandCorpusScheduler,
    },
    events::{setup_restarting_mgr_std, Event, EventManager, EventRestarter, LogSeverity},
    executors::{inprocess::InProcessExecutor, ExitKind, TimeoutExecutor},
    feedback_or,
    feedbacks::{
        CrashFeedback, FeedbackStatesTuple, MapFeedbackState, MapIndexesMetadata, MaxMapFeedback,
        MaxReducer, TimeFeedback, TimeoutFeedback,
    },
    fuzzer::{Fuzzer, StdFuzzer},
    inputs::BytesInput,
    mutators::{
        havoc_mutations,
        scheduled::{tokens_mutations, StdScheduledMutator},
        token_mutations::Tokens,
    },
    observers::{HitcountsMapObserver, StdMapObserver, TimeObserver},
    stages::mutational::StdMutationalStage,
    state::{HasCorpus, HasMetadata, StdState},
    stats::{MultiStats, SimpleStats},
    Error, Evaluator,
};

use crate::fuzzer::error_observer::ErrorObserver;
use crate::fuzzer::mutations::trace_mutations;
use crate::fuzzer::stages::{PuffinMutationalStage, PuffinScheduledMutator};
use crate::openssl_binding::make_deterministic;

use super::harness;
use super::{EDGES_MAP, MAX_EDGES_NUM};

/// Default value, how many iterations each stage gets, as an upper bound
/// It may randomly continue earlier. Each iteration works on a different Input from the corpus
pub static MAX_ITERATIONS_PER_STAGE: u64 = 256;
pub static MAX_MUTATIONS_PER_ITERATION: u64 = 16;

static STATS_COUNTER: AtomicUsize = AtomicUsize::new(0);

/// Starts the fuzzing loop
pub fn start(num_cores: usize, corpus_dirs: &[PathBuf], objective_dir: &PathBuf, broker_port: u16) {
    info!("Running on {} cores", num_cores);

    make_deterministic();
    let shmem_provider = StdShMemProvider::new().expect("Failed to init shared memory");

    let stats = MultiStats::new(|s| {
        let log_count = STATS_COUNTER.fetch_add(1, Ordering::SeqCst);
        // GLOBAL and CLIENT message
        if log_count % 1000 == 0 || (log_count - 1) % 1000 == 0 {
            info!("{}", s)
        }
    });

    let mut run_client = |state: Option<StdState<_, _, _, _, _>>, mut restarting_mgr| {
        info!("We're a client, let's fuzz :)");

        let edges_observer = HitcountsMapObserver::new(StdMapObserver::new("edges", unsafe {
            &mut EDGES_MAP[0..MAX_EDGES_NUM]
        }));
        let time_observer = TimeObserver::new("time");
        let error_observer = ErrorObserver::new("error");

        let edges_feedback_state = MapFeedbackState::with_observer(&edges_observer);

        let feedback = feedback_or!(
            // New maximization map feedback linked to the edges observer and the feedback state
            // `track_indexes` needed because of IndexesLenTimeMinimizerCorpusScheduler
            MaxMapFeedback::new_tracking(&edges_feedback_state, &edges_observer, true, false),
            // Time feedback, this one does not need a feedback state
            // needed for IndexesLenTimeMinimizerCorpusScheduler
            TimeFeedback::new_with_observer(&time_observer)
        );

        // A feedback to choose if an input is a solution or not
        let objective = feedback_or!(CrashFeedback::new(), TimeoutFeedback::new());

        // If not restarting, create a State from scratch
        let mut state = state.unwrap_or_else(|| {
            let seed = current_nanos();
            warn!("Seed is {}", seed);
            StdState::new(
                StdRand::with_seed(seed),
                InMemoryCorpus::new(),
                OnDiskCorpus::new(objective_dir.clone()).unwrap(),
                // They are the data related to the feedbacks that you want to persist in the State.
                tuple_list!(edges_feedback_state),
            )
        });

        let mutator = PuffinScheduledMutator::new(trace_mutations(), MAX_MUTATIONS_PER_ITERATION);
        let mut stages = tuple_list!(PuffinMutationalStage::new(mutator, MAX_ITERATIONS_PER_STAGE));

        // A minimization+queue policy to get testcasess from the corpus
        let scheduler = IndexesLenTimeMinimizerCorpusScheduler::new(QueueCorpusScheduler::new());
        //let scheduler = RandCorpusScheduler::new();
        let mut fuzzer = StdFuzzer::new(scheduler, feedback, objective);

        let mut harness_fn = &mut harness::harness;

        let mut executor = TimeoutExecutor::new(
            InProcessExecutor::new(
                &mut harness_fn,
                // hint: edges_observer is expensive to serialize (only noticeable if we add all inputs to the corpus)
                tuple_list!(edges_observer, time_observer, error_observer),
                &mut fuzzer,
                &mut state,
                &mut restarting_mgr,
            )?,
            Duration::new(2, 0),
        );

        // In case the corpus is empty (on first run), reset
        if state.corpus().count() < 1 {
            state
                .load_initial_inputs(
                    &mut fuzzer,
                    &mut executor,
                    &mut restarting_mgr,
                    &corpus_dirs,
                )
                .unwrap_or_else(|err| {
                    panic!(
                        "Failed to load initial corpus at {:?}: {}",
                        &corpus_dirs, err
                    )
                });
            println!("We imported {} inputs from disk.", state.corpus().count());
        }

        fuzzer.fuzz_loop(&mut stages, &mut executor, &mut state, &mut restarting_mgr)?;
        Ok(())
    };

    libafl::bolts::launcher::Launcher::builder()
        .shmem_provider(shmem_provider)
        .stats(stats)
        .run_client(&mut run_client)
        .cores(&(0..num_cores).collect_vec()) // possibly replace by parse_core_bind_arg
        .broker_port(broker_port)
        //todo where should we log the output of the harness?
        .stdout_file(Some("/dev/null"))
        .build()
        .launch()
        .expect("Launcher failed");
}
