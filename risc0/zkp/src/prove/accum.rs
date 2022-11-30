use std::{collections::BTreeMap, sync::Mutex};

use anyhow::Result;

use crate::{
    adapter::CircuitStepHandler,
    field::{Elem, ExtElem, Field},
};

/// Tracks plonk accumulations for an execution.
pub struct Accum<E: Elem> {
    /// Total number of cycles in this run.
    cycles: usize,

    // Plonk kinds and their data.
    kinds: BTreeMap<String, Vec<E>>,
}

impl<E: Elem> Accum<E> {
    pub fn new(cycles: usize) -> Self {
        Accum {
            cycles,
            kinds: BTreeMap::new(),
        }
    }

    pub fn calc_prefix_products(&mut self) {
        for (_kind, elems) in self.kinds.iter_mut() {
            let mut tot = E::ONE;
            for elem in elems.iter_mut() {
                tot = tot * *elem;
                *elem = tot;
            }
        }
    }

    fn get_ptr(&mut self, kind: String) -> *mut E {
        self.kinds
            .entry(kind)
            .or_insert_with(|| vec![E::ONE; self.cycles])
            .as_mut_ptr()
    }
}

pub struct Handler<'a, F: Field> {
    p: &'a Mutex<Accum<F::ExtElem>>,
    cycles: usize,
    kinds: BTreeMap<String, *mut F::ExtElem>,
}

impl<'a, F: Field> Handler<'a, F> {
    pub fn new(p: &'a Mutex<Accum<F::ExtElem>>) -> Self {
        let cycles = p.lock().unwrap().cycles;
        Handler {
            p,
            kinds: BTreeMap::new(),
            cycles,
        }
    }

    fn get_ptr(&mut self, kind: &str) -> *mut F::ExtElem {
        if let Some(entry) = self.kinds.get_mut(kind) {
            *entry
        } else {
            let mut p = self.p.lock().unwrap();
            let ptr = p.get_ptr(kind.to_string());
            self.kinds.insert(kind.to_string(), ptr);
            ptr
        }
    }
}

impl<'a, F: Field> CircuitStepHandler<F::Elem> for Handler<'a, F> {
    fn call(
        &mut self,
        cycle: usize,
        name: &str,
        extra: &str,
        args: &[F::Elem],
        outs: &mut [F::Elem],
    ) -> Result<()> {
        assert!(cycle < self.cycles);
        match name {
            "plonkWriteAccum" => {
                assert_eq!(args.len(), F::ExtElem::EXT_SIZE);
                let elem = F::ExtElem::from_subelems(args.iter().copied());
                let ptr = self.get_ptr(extra);
                // Already checked that our cycle number is in range, so this offset is in the
                // buffer.
                unsafe { ptr.add(cycle).write(elem) };
            }
            "plonkReadAccum" => {
                assert_eq!(outs.len(), F::ExtElem::EXT_SIZE);
                let ptr = self.get_ptr(extra);
                // Already checked that our cycle number is in range, so this offset is in the
                // buffer.
                let elem = unsafe { ptr.add(cycle).read() };
                outs.clone_from_slice(elem.subelems());
            }
            _ => panic!("Unknown accum operation {name}"),
        }
        Ok(())
    }

    fn sort(&mut self, _name: &str) {
        unimplemented!()
    }

    fn calc_prefix_products(&mut self) {
        unimplemented!()
    }
}