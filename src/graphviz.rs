//! This module adds plotting capabilities to[`Term`]sand Traces. The output of the functions in
//! this module can be passed to the command line utility `dot` which is part of graphviz.

use crate::term::{remove_prefix, Term, remove_fn_prefix};
use crate::trace::{Action, Trace};
use itertools::Itertools;
use std::io::{ErrorKind, Write};
use std::process::{Command, Stdio};
use std::{fmt, io};

// Colorful theme
/*const FONT: &'static str = "Latin Modern Roman";
const SHAPE: &'static str = "box";
const SHAPE_LEAVES: &'static str = "oval";
const STYLE: &'static str = "filled";
const COLOR: &'static str = "2";
const COLOR_LEAVES: &'static str = "1";
const SHOW_LABELS: bool = false */

// Thesis theme
const FONT: &'static str = "Latin Modern Roman";
const SHAPE: &'static str = "none";
const SHAPE_LEAVES: &'static str = "none";
const STYLE: &'static str = "";
const COLOR: &'static str = "#00000000";
const COLOR_LEAVES: &'static str = "#00000000";
const SHOW_LABELS: bool = false;

pub fn write_graphviz(output: &str, format: &str, dot_script: &str) -> Result<(), io::Error> {
    let mut child = Command::new("dot")
        .args(&["-o", output, "-T", format])
        .stdin(Stdio::piped())
        .spawn()
        .expect("failed to spawn dot");

    let mut dot_stdin = child
        .stdin
        .take()
        .ok_or(io::Error::new(ErrorKind::Other, "Failed to open stdin"))?;
    dot_stdin.write_all(dot_script.as_bytes().as_ref())?;
    drop(dot_stdin);
    child.wait().expect("failed to execute dot");
    Ok(())
}

impl Trace {
    pub fn dot_graph(&self, tree_mode: bool) -> String {
        format!(
            "strict digraph \"Trace\" \
            {{ \
                splines=false;\
                fontname=\"{}\";\
                {} \
            }}",
            FONT,
            self.dot_subgraphs(tree_mode).join("\n")
        )
    }

    pub fn dot_subgraphs(&self, tree_mode: bool) -> Vec<String> {
        let mut subgraphs = Vec::new();

        for (i, step) in self.steps.iter().enumerate() {
            let subgraph_name = format!("Step #{} (Agent  {})", i, step.agent);

            let subgraph = match &step.action {
                Action::Input(input) => {
                    format!(
                        "{}",
                        input
                            .recipe
                            .dot_subgraph(tree_mode, i, subgraph_name.as_str())
                    )
                }
                Action::Output(_) => format!(
                    "subgraph cluster{} \
                    {{ \
                        peripheries=0;\
                        label=\"{label}\";\
                        \"\" [color=\"#00000000\"];\
                    }}",
                    i,
                    label=(if SHOW_LABELS { subgraph_name.as_str() } else {""}),
                ),
            };

            subgraphs.push(subgraph);
        }

        subgraphs
    }
}

impl Term {
    fn unique_id(&self, tree_mode: bool, cluster_id: usize) -> String {
        match self {
            Term::Variable(variable) => {
                if tree_mode {
                    format!("v_{}_{}", cluster_id, variable.unique_id)
                } else {
                    format!("v_{}", variable.resistant_id)
                }
            }
            Term::Application(func, _) => {
                if tree_mode {
                    format!("f_{}_{}", cluster_id, func.unique_id)
                } else {
                    format!("f_{}", func.resistant_id)
                }
            }
        }
    }

    fn node_attributes(displayable: impl fmt::Display, color: &str, shape: &str) -> String {
        format!(
            "[label=\"{}\",style=\"{style}\",colorscheme=dark28,fillcolor=\"{}\",shape=\"{}\"]",
            displayable, color, shape,
            style=STYLE
        )
    }

    fn collect_statements(
        term: &Term,
        tree_mode: bool,
        cluster_id: usize,
        statements: &mut Vec<String>,
    ) {
        match term {
            Term::Variable(variable) => {
                statements.push(format!(
                    "{} {} [fontname=\"{}\"];",
                    term.unique_id(tree_mode, cluster_id),
                    Self::node_attributes(variable, COLOR_LEAVES, SHAPE_LEAVES),
                    FONT
                ));
            }
            Term::Application(func, subterms) => {
                statements.push(format!(
                    "{} {} [fontname=\"{}\"];",
                    term.unique_id(tree_mode, cluster_id),
                    Self::node_attributes(
                        remove_fn_prefix(&remove_prefix(func.name())),
                        if func.arity() == 0 { COLOR_LEAVES } else { COLOR },
                        if func.arity() == 0 { SHAPE_LEAVES} else { SHAPE }
                    ),
                    FONT
                ));

                for subterm in subterms {
                    statements.push(format!(
                        "{} -> {};",
                        term.unique_id(tree_mode, cluster_id),
                        subterm.unique_id(tree_mode, cluster_id)
                    ));
                    Self::collect_statements(subterm, tree_mode, cluster_id, statements);
                }
            }
        }
    }

    /// If `tree_mode` is true then each subgraph is self-contained and does not reference other
    /// clusters or nodes outside of this subgraph. Therefore, only trees are generated. If it is
    /// false, then graphs are rendered.
    pub fn dot_subgraph(&self, tree_mode: bool, cluster_id: usize, label: &str) -> String {
        let mut statements = Vec::new();
        Self::collect_statements(self, tree_mode, cluster_id, &mut statements);
        format!(
            "subgraph cluster{} \
            {{ \
               	peripheries=0;\
                fontname=\"{font}\";\
                label=\"{label}\";\
                \n{}\n\
            }}",
            cluster_id,
            statements.iter().join("\n"),
            label=(if SHOW_LABELS { label } else {""}),
            font=FONT,
        )
    }
}

#[cfg(test)]
mod tests {
    use crate::agent::AgentName;
    use crate::fuzzer::seeds::seed_client_attacker12;

    #[test]
    fn test_dot_graph() {
        let server = AgentName::first();
        let trace = seed_client_attacker12(server);
        println!("{}", trace.dot_graph(true));
    }
}
