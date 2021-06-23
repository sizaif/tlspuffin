use std::ops::Deref;

use libafl::bolts::rands::{Rand, RomuDuoJrRand, RomuTrioRand, StdRand};
use libafl::corpus::InMemoryCorpus;
use libafl::mutators::{MutationResult, Mutator};
use libafl::state::StdState;
use openssl::rand::rand_bytes;

use crate::agent::AgentName;
use crate::fuzzer::mutations::{
    RepeatMutator, ReplaceReuseMutator,
};
use crate::fuzzer::seeds::*;
use crate::graphviz::write_graphviz;
use crate::openssl_binding::make_deterministic;
use crate::term::{Symbol, Term};
use crate::trace::{Action, InputAction, Step, Trace, TraceContext};

#[test]
fn test_openssl_no_randomness() {
    make_deterministic(); // his affects also other tests, which is fine as we generally prefer deterministic tests
    let mut buf1 = [0; 2];
    rand_bytes(&mut buf1).unwrap();
    assert_eq!(buf1, [70, 100]);
}

/// Checks whether repeat can repeat the last step
#[test]
fn test_repeat_cve() {
    let rand = StdRand::with_seed(1235);
    let corpus: InMemoryCorpus<Trace> = InMemoryCorpus::new();
    let mut state = StdState::new(rand, corpus, InMemoryCorpus::new(), ());
    let client = AgentName::first();
    let server = client.next();
    let _trace = seed_client_attacker12(client, server);

    let mut mutator = RepeatMutator::new();

    fn check_is_encrypt12(step: &Step) -> bool {
        if let Action::Input(input) = &step.action {
            if input.recipe.root_node().unwrap().data().name() == "tlspuffin::tls::fn_utils::fn_encrypt12" {
                return true;
            }
        }
        false
    }

    loop {
        let mut trace = seed_client_attacker12(client, server);
        mutator.mutate(&mut state, &mut trace, 0).unwrap();

        let length = trace.steps.len();
        if let Some(last) = trace.steps.get(length - 1) {
            if check_is_encrypt12(last) {
                if let Some(step) = trace.steps.get(length - 2) {
                    if check_is_encrypt12(step) {
                        break;
                    }
                }
            }
        }
    }
}

fn plot(trace: &Trace, i: u16) {
    write_graphviz(
        format!("test_mutation{}.svg", i).as_str(),
        "svg",
        trace.dot_graph(true).as_str(),
    )
    .unwrap();
}
/*
#[test]
fn test_replace_match_cve() {
    let rand = StdRand::with_seed(1235);
    let corpus: InMemoryCorpus<Trace> = InMemoryCorpus::new();
    let mut state = StdState::new(rand, corpus, InMemoryCorpus::new(), ());
    let client = AgentName::first();
    let server = client.next();
    let _trace = seed_client_attacker12(client, server);

    let mut mutator = ReplaceMatchMutator::new();

    loop {
        let mut trace = seed_client_attacker12(client, server);
        mutator.mutate(&mut state, &mut trace, 0).unwrap();

        if let Some(last) = trace.steps.iter().last() {
            match &last.action {
                Action::Input(input) => {
                    if count_functions_term(
                        &input.recipe,
                        "tlspuffin::tls::fn_constants::fn_seq_0",
                    ) == 0 {
                        break;
                    }
                }

                Action::Output(_) => {}
            }
        }
    }
}*/

fn count_functions(trace: &Trace, find_name: &'static str) -> u16 {
    trace
        .steps
        .iter()
        .map(|step| match &step.action {
            Action::Input(input) => count_functions_term(&input.recipe, find_name),
            Action::Output(_) => 0,
        })
        .sum::<u16>()
}

fn count_functions_term(term: &Term, find_name: &'static str) -> u16 {
    let mut extension_appends = 0;
    for node in term.traverse_from_root().unwrap() {
        if let Symbol::Application(func) = node.data() {
            if func.name() == find_name {
                extension_appends += 1;
            }
        }
    }
    extension_appends
}
/*
#[test]
fn test_remove_lift_removes_extensions() {
    let rand = StdRand::with_seed(1235);
    let corpus: InMemoryCorpus<Trace> = InMemoryCorpus::new();
    let mut state = StdState::new(rand, corpus, InMemoryCorpus::new(), ());
    let client = AgentName::first();
    let server = client.next();
    let _trace = seed_client_attacker12(client, server);

    let mut mutator = RemoveAndLiftMutator::new();

    // Returns the amount of extensions in the trace
    fn sum_extension_appends(trace: &Trace) -> u16 {
        count_functions(
            trace,
            "tlspuffin::tls::fn_extensions::fn_client_extensions_append",
        )
    }

    loop {
        let mut trace = seed_client_attacker12(client, server);
        let before_mutation = sum_extension_appends(&trace);
        //plot(&trace);
        let result = mutator.mutate(&mut state, &mut trace, 0).unwrap();

        if let MutationResult::Mutated = result {
            let after_mutation = sum_extension_appends(&trace);
            if after_mutation < before_mutation {
                //plot(&trace);
                break;
            }
        }
    }
}
*/
#[test]
fn test_replace_reuse() {
    let rand = StdRand::with_seed(45);
    let corpus: InMemoryCorpus<Trace> = InMemoryCorpus::new();
    let mut state = StdState::new(rand, corpus, InMemoryCorpus::new(), ());
    let client = AgentName::first();
    let server = client.next();
    let mut mutator = ReplaceReuseMutator::new();

    fn count_client_hello(trace: &Trace) -> u16 {
        count_functions(trace, "tlspuffin::tls::fn_messages::fn_client_hello")
    }

    fn count_finished(trace: &Trace) -> u16 {
        count_functions(trace, "tlspuffin::tls::fn_messages::fn_finished")
    }

    loop {
        let mut trace = seed_client_attacker12(client, server);
        //let before_mutation = count_client_hello(&trace);
        let result = mutator.mutate(&mut state, &mut trace, 0).unwrap();

        if let MutationResult::Mutated = result {
            let after_mutation = count_client_hello(&trace);
            if after_mutation == 2 && count_finished(&trace) == 0 {
                //plot(&trace, 0);
                break;
            }
        }
    }
}

// this should trigger the cve and crash soon
/*#[test]
fn test_reach_cve_through_extension_removal() {
    let rand = StdRand::with_seed(1235);
    let corpus: InMemoryCorpus<Trace> = InMemoryCorpus::new();
    let mut state = StdState::new(rand, corpus, InMemoryCorpus::new(), ());
    let client = AgentName::first();
    let server = client.next();
    let _trace = seed_client_attacker12(client, server);

    let mut mutator = RemoveAndLiftMutator::new();

    loop {
        let mut trace = seed_almost_cve_2021_3449(client, server);
        mutator.mutate(&mut state, &mut trace, 0).unwrap();

        let mut ctx = TraceContext::new();
        trace.spawn_agents(&mut ctx);
        trace.execute(&mut ctx);
    }
}*/

#[test]
fn test_rand() {
    let mut rand = RomuDuoJrRand::with_seed(1337);
    assert_ne!(rand.between(0, 1), rand.between(0, 1))
}
