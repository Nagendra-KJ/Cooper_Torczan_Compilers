use petgraph::dot::Dot;
use petgraph::graph::DiGraph;
use std::collections::{HashMap, HashSet};
use std::fs::File;
use std::io::Write;
use std::process::Command;

use crate::fa::{FAState, Symbol, FA};
use crate::reg_ex::{Base, Factor, Quantifier, RegEx, Term};

#[derive(Debug, Clone)]
struct NFAState {
    id: usize,
    transitions: HashMap<Symbol, HashSet<usize>>, // Store by reference is not a thing in Rust
}

#[derive(Debug)]
struct NFA {
    states: Vec<NFAState>,
    start_state: usize,
    accept_states: HashSet<usize>,
    alphabet: HashSet<char>,
}

impl FA for NFA {
    fn show_fa(&self, filename: &str) {
        let mut graph = DiGraph::new();
        let mut node_map = std::collections::HashMap::new();

        // Add nodes
        for state in &self.states {
            let node = graph.add_node(format!("State {}", state.id));
            node_map.insert(state.id, node);
        }

        // Add edges
        for state in &self.states {
            for (symbol, targets) in &state.transitions {
                for target in targets {
                    let symbol_str = match symbol {
                        Symbol::Char(c) => c.to_string(),
                        Symbol::Epsilon => "𝛆".to_string(),
                    };
                    graph.add_edge(node_map[&state.id], node_map[&target], symbol_str);
                }
            }
        }

        // Mark Start and Accept States

        let start_node = node_map[&self.start_state];
        graph[start_node] = format!("Start\nState {}", self.start_state);

        for accept in &self.accept_states {
            let accept_node = node_map[&accept];
            graph[accept_node] = format!("Accept\nState {}", accept);
        }

        let dot = Dot::new(&graph);

        // Write dot to file
        let dot_filename = format!("{}.dot", filename);
        let mut dot_file = File::create(&dot_filename).expect("Failed to create dot file");

        dot_file
            .write_all(dot.to_string().as_bytes())
            .expect("Failed to write dot file");

        Command::new("dot")
            .args(&["-Tjpg", &dot_filename, "-o", &format!("{}.jpg", filename)])
            .output()
            .expect("Failed to execute Graphviz");

        println!("FA vizualization saved as {}.jpg", filename);
    }

    fn add_transition(&mut self, from: usize, symbol: Symbol, to: usize) {
        self.states[from].add_transition(symbol, to);
    }

    fn set_accept_state(&mut self, state_id: usize) {
        self.accept_states.insert(state_id);
    }

    fn add_state(&mut self) -> usize {
        let state_id = self.states.len();
        let new_state: NFAState = NFAState::new(state_id);
        self.states.push(new_state.clone());
        return state_id;
    }
}

impl FAState for NFAState {
    fn add_transition(&mut self, symbol: Symbol, to: usize) {
        self.transitions.entry(symbol).or_default().insert(to);
    }
}

impl NFAState {
    fn new(id: usize) -> Self {
        NFAState {
            id,
            transitions: HashMap::new(),
        }
    }
}

impl NFA {
    fn new() -> Self {
        NFA {
            states: Vec::new(),
            start_state: 0,
            accept_states: HashSet::new(),
            alphabet: HashSet::new(),
        }
    }

    fn alternation(nfa1: NFA, nfa2: NFA) -> NFA {
        let mut result = NFA::new();
        let new_start = result.add_state();

        // Copy states from NFA 1
        let offset1 = result.states.len();

        for mut state in nfa1.states {
            state.id += offset1;
            let mut new_transitions = HashMap::new();

            for (symbol, targets) in state.transitions {
                let mut new_targets = HashSet::new();

                for target in targets {
                    new_targets.insert(target + offset1);
                }
                new_transitions.insert(symbol, new_targets);
            }
            state.transitions = new_transitions;
            result.states.push(state);
        }

        // Add epsilon transition from new start to start state of NFA1
        result.add_transition(new_start, Symbol::Epsilon, nfa1.start_state + offset1);

        let offset2 = result.states.len();

        for mut state in nfa2.states {
            // Copy states from NFA2
            state.id += offset2;
            let mut new_transitions = HashMap::new();

            for (symbol, targets) in state.transitions {
                let mut new_targets = HashSet::new();

                for target in targets {
                    new_targets.insert(target + offset2);
                }
                new_transitions.insert(symbol, new_targets);
            }
            state.transitions = new_transitions;
            result.states.push(state);
        }

        // Add epsilon transition from new start to start state of NFA2
        result.add_transition(new_start, Symbol::Epsilon, nfa2.start_state + offset2);

        let new_accept = result.add_state();

        // Add epsilon transitions from NFA1s accept states to new accept
        for accept_state in nfa1.accept_states {
            result.add_transition(accept_state + offset1, Symbol::Epsilon, new_accept);
        }

        // Add epsilon transitions from NFA2s accept states to new accept
        for accept_state in nfa2.accept_states.clone() {
            result.add_transition(accept_state + offset2, Symbol::Epsilon, new_accept);
        }

        result.start_state = new_start;
        result.set_accept_state(new_accept);
        result.alphabet = nfa1.alphabet.union(&nfa2.alphabet).cloned().collect();

        return result;
    }

    fn closure(nfa: NFA, quantifier: Quantifier) -> NFA {
        let mut result = NFA::new();
        let new_start = result.add_state(); // Add a new start and accept state

        // Copy states from the original NFA

        let offset = result.states.len();

        for mut state in nfa.states {
            state.id += offset;
            let mut new_transitions = HashMap::new();

            for (symbol, targets) in state.transitions {
                let mut new_targets = HashSet::new();

                for target in targets {
                    new_targets.insert(target + offset);
                }
                new_transitions.insert(symbol, new_targets);
            }
            state.transitions = new_transitions;
            result.states.push(state);
        }

        result.add_transition(new_start, Symbol::Epsilon, nfa.start_state + offset); // Add epsilon
                                                                                     // transitions
                                                                                     // from new
                                                                                     // start to
                                                                                     // old start
        let new_accept = result.add_state();
        match quantifier {
            Quantifier::Star | Quantifier::Question => {
                result.add_transition(new_start, Symbol::Epsilon, new_accept); // Add epsilon transitions
                                                                               // from new start to new
                                                                               // accept state
            }
            Quantifier::Plus => {}
        }

        for accept in nfa.accept_states {
            // Add epsilon transitions from old accept to new accept
            // and old accept and old start
            match quantifier {
                Quantifier::Star | Quantifier::Plus => {
                    result.add_transition(
                        accept + offset,
                        Symbol::Epsilon,
                        nfa.start_state + offset,
                    );
                }
                _ => {}
            }
            result.add_transition(accept + offset, Symbol::Epsilon, new_accept);
        }

        result.start_state = new_start; // Set new start and new accepts
        result.set_accept_state(new_accept);
        return result;
    }

    fn concatenate(nfa1: NFA, nfa2: NFA) -> NFA {
        let mut result: NFA = NFA::new();
        result.states = nfa1.states.clone(); // Clone all states from nfa1
        let offset = nfa1.states.len();

        // Add states and their transitions from nfa2 into the resultant nfa
        for mut state in nfa2.states {
            // For each state in NFA2
            state.id += offset; // Change their ID by offset
            let mut new_transitions = HashMap::new(); // Create new transitions

            for (symbol, targets) in state.transitions {
                let mut new_targets = HashSet::new();

                for target in targets {
                    new_targets.insert(target + offset);
                }
                new_transitions.insert(symbol, new_targets);
            }
            state.transitions = new_transitions; // Add the new transitions to new states
            result.states.push(state); // Add the new states to the results
        }

        // Add epsilon transitions from each acceptor state of NFA1 to start state of NFA2

        for accept_id in nfa1.accept_states {
            result.add_transition(accept_id, Symbol::Epsilon, nfa2.start_state + offset);
        }

        result.start_state = nfa1.start_state; // Make the start state of NFA1 the start state of
                                               // the result
        result.accept_states = nfa2.accept_states.into_iter().map(|s| s + offset).collect(); // Make the accept states of NFA2 the accept
                                                                                             // states of the result
        result.alphabet = nfa1.alphabet.union(&nfa2.alphabet).cloned().collect();
        return result;
    }

    fn literal_construction(character: char) -> NFA {
        let mut result: NFA = NFA::new();
        let start_state = result.add_state();
        let end_state = result.add_state();
        result.alphabet.insert(character);
        result.add_transition(start_state, Symbol::Char(character), end_state);

        result.start_state = start_state;
        result.set_accept_state(end_state);
        return result;
    }
}

fn parse_base_tree(tree: Base) -> NFA {
    match tree {
        Base::Character(character) => NFA::literal_construction(character),
        Base::EscapeCharacter(character) => NFA::literal_construction(character),
        Base::Exp(regex) => {
            let regex = *regex;
            parse_regex_tree(regex)
        }
    }
}

fn parse_factor_tree(tree: Factor) -> NFA {
    match tree {
        Factor::SimpleFactor(base, quantifier) => {
            let nfa = parse_base_tree(base);
            match quantifier {
                None => nfa,
                Some(quantifier) => NFA::closure(nfa, quantifier),
            }
        }
    }
}

fn parse_term_tree(tree: Term) -> NFA {
    match tree {
        Term::SimpleTerm(factor) => parse_factor_tree(factor),
        Term::ConcatTerm(rfactor, lterm) => {
            let lterm = *lterm;
            let nfa1 = parse_term_tree(lterm);
            let nfa2 = parse_factor_tree(rfactor);
            NFA::concatenate(nfa1, nfa2)
        }
    }
}

fn parse_regex_tree(tree: RegEx) -> NFA {
    match tree {
        RegEx::SimpleRegex(term) => parse_term_tree(term),
        RegEx::AlterRegex(lterm, rregex) => {
            let rregex = *rregex; // Unboxing the value
            let nfa1 = parse_term_tree(lterm);
            let nfa2 = parse_regex_tree(rregex);
            NFA::alternation(nfa1, nfa2)
        }
    }
}

pub fn construct_nfa(regex: &str, syntax_tree: RegEx) {
    let nfa = parse_regex_tree(syntax_tree);
    nfa.show_fa(regex);
}
