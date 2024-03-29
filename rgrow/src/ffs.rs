#![allow(clippy::too_many_arguments)]

use std::sync::Arc;

use crate::base::{GrowError, RgrowError, Tile};
use crate::canvas::{CanvasPeriodic, CanvasSquare, CanvasTube, PointSafe2};
use crate::models::ktam::KTAM;
use crate::models::oldktam::OldKTAM;
use crate::state::{NullStateTracker, QuadTreeState, StateTracked};
use crate::system::{EvolveBounds, SystemWithDimers};
use crate::tileset::{CanvasType, FromTileSet, Model, TileSet, SIZE_DEFAULT};

use super::*;
//use ndarray::prelude::*;
//use ndarray::Zip;
use base::{NumTiles, Rate};

use ndarray::ArrayView2;
#[cfg(feature = "python")]
use numpy::{PyArray2, ToPyArray};
use rand::{distributions::Uniform, distributions::WeightedIndex, prelude::Distribution};
use rand::{prelude::SmallRng, SeedableRng};
use rand::{thread_rng, Rng};

#[cfg(feature = "python")]
use pyo3::exceptions::PyTypeError;
#[cfg(feature = "python")]
use pyo3::prelude::*;

use state::{State, StateWithCreate};

use system::{Orientation, System};
//use std::convert::{TryFrom, TryInto};

/// Configuration options for FFS.
#[derive(Debug, Clone)]
#[cfg_attr(feature = "python", pyclass(get_all, set_all))]
pub struct FFSRunConfig {
    /// Use constant-variance, variable-configurations-per-surface method.
    /// If false, use max_configs for each surface.
    pub constant_variance: bool,
    /// Variance per mean^2 for constant-variance method.
    pub var_per_mean2: f64,
    /// Minimum number of configuratons to generate at each level.
    pub min_configs: usize,
    /// Maximum number of configurations to generate at each level.
    pub max_configs: usize,
    /// Use early cutoff for constant-variance method.
    pub early_cutoff: bool,
    pub cutoff_probability: f64,
    pub cutoff_number: usize,
    pub min_cutoff_size: NumTiles,
    pub init_bound: EvolveBounds,
    pub subseq_bound: EvolveBounds,
    pub start_size: NumTiles,
    pub size_step: NumTiles,
    pub keep_configs: bool,
    pub min_nuc_rate: Option<Rate>,
    pub canvas_size: (usize, usize),
    pub target_size: NumTiles,
}

impl Default for FFSRunConfig {
    fn default() -> Self {
        Self {
            constant_variance: true,
            var_per_mean2: 0.01,
            min_configs: 1000,
            max_configs: 100000,
            early_cutoff: true,
            cutoff_probability: 0.99,
            cutoff_number: 4,
            min_cutoff_size: 30,
            init_bound: EvolveBounds::default().for_time(1e7),
            subseq_bound: EvolveBounds::default().for_time(1e7),
            start_size: 3,
            size_step: 1,
            keep_configs: false,
            min_nuc_rate: None,
            canvas_size: (64, 64),
            target_size: 100,
        }
    }
}

#[cfg(feature = "python")]
impl FFSRunConfig {
    pub fn _py_set(&mut self, k: &str, v: &PyAny, _py: Python) -> PyResult<()> {
        match k {
            "constant_variance" => self.constant_variance = v.extract()?,
            "var_per_mean2" => self.var_per_mean2 = v.extract()?,
            "min_configs" => self.min_configs = v.extract()?,
            "max_configs" => self.max_configs = v.extract()?,
            "early_cutoff" => self.early_cutoff = v.extract()?,
            "cutoff_probability" => self.cutoff_probability = v.extract()?,
            "cutoff_number" => self.cutoff_number = v.extract()?,
            "min_cutoff_size" => self.min_cutoff_size = v.extract()?,
            "init_bound" => self.init_bound = v.extract()?,
            "subseq_bound" => self.subseq_bound = v.extract()?,
            "start_size" => self.start_size = v.extract()?,
            "size_step" => self.size_step = v.extract()?,
            "keep_configs" => self.keep_configs = v.extract()?,
            "min_nuc_rate" => self.min_nuc_rate = v.extract()?,
            "canvas_size" => self.canvas_size = v.extract()?,
            "target_size" => self.target_size = v.extract()?,
            _ => {
                return Err(PyTypeError::new_err(format!(
                    "Unknown FFSRunConfig setting: {k}"
                )))
            }
        };
        Ok(())
    }
}

#[cfg(feature = "python")]
#[pymethods]
impl FFSRunConfig {
    #[new]
    fn new(
        constant_variance: Option<bool>,
        var_per_mean2: Option<f64>,
        min_configs: Option<usize>,
        max_configs: Option<usize>,
        early_cutoff: Option<bool>,
        cutoff_probability: Option<f64>,
        cutoff_number: Option<usize>,
        min_cutoff_size: Option<NumTiles>,
        init_bound: Option<EvolveBounds>,
        subseq_bound: Option<EvolveBounds>,
        start_size: Option<NumTiles>,
        size_step: Option<NumTiles>,
        keep_configs: Option<bool>,
        min_nuc_rate: Option<Rate>,
        canvas_size: Option<(usize, usize)>,
        target_size: Option<NumTiles>,
    ) -> Self {
        let mut rc = Self::default();

        if let Some(x) = constant_variance {
            rc.constant_variance = x;
        }

        if let Some(x) = var_per_mean2 {
            rc.var_per_mean2 = x;
        }

        if let Some(x) = min_configs {
            rc.min_configs = x;
        }
        if let Some(x) = max_configs {
            rc.max_configs = x;
        }
        if let Some(x) = early_cutoff {
            rc.early_cutoff = x;
        }
        if let Some(x) = cutoff_probability {
            rc.cutoff_probability = x;
        }
        if let Some(x) = cutoff_number {
            rc.cutoff_number = x;
        }
        if let Some(x) = min_cutoff_size {
            rc.min_cutoff_size = x;
        }
        if let Some(x) = init_bound {
            rc.init_bound = x;
        }
        if let Some(x) = subseq_bound {
            rc.subseq_bound = x;
        }
        if let Some(x) = start_size {
            rc.start_size = x;
        }
        if let Some(x) = size_step {
            rc.size_step = x;
        }
        if let Some(x) = keep_configs {
            rc.keep_configs = x;
        }

        rc.min_nuc_rate = min_nuc_rate;

        if let Some(x) = canvas_size {
            rc.canvas_size = x;
        }
        if let Some(x) = target_size {
            rc.target_size = x;
        }
        rc
    }
}

pub trait FFSResult: Send + Sync {
    fn nucleation_rate(&self) -> f64;
    fn forward_vec(&self) -> &Vec<f64>;
    fn dimerization_rate(&self) -> f64;
    fn surfaces(&self) -> Vec<&dyn FFSSurface>;
}

pub trait FFSSurface: Send + Sync {
    fn get_config(&self, i: usize) -> ArrayView2<Tile>;
    fn configs(&self) -> Vec<ArrayView2<Tile>> {
        (0..self.num_configs())
            .map(|i| self.get_config(i))
            .collect()
    }
    fn previous_list(&self) -> Vec<usize>;
    fn num_configs(&self) -> usize;
    fn num_trials(&self) -> usize;
    fn target_size(&self) -> NumTiles;
}

impl TileSet {
    pub fn run_ffs(&self, config: &FFSRunConfig) -> Result<Box<dyn FFSResult>, RgrowError> {
        match self.model.unwrap_or(Model::KTAM) {
            Model::KTAM => match self.canvas_type.unwrap_or(CanvasType::Periodic) {
                CanvasType::Square => Ok(Box::new(FFSRun::<
                    QuadTreeState<CanvasSquare, NullStateTracker>,
                >::create_from_tileset::<KTAM>(
                    self, config
                )?)),
                CanvasType::Periodic => Ok(Box::new(FFSRun::<
                    QuadTreeState<CanvasPeriodic, NullStateTracker>,
                >::create_from_tileset::<KTAM>(
                    self, config
                )?)),
                CanvasType::Tube => Ok(Box::new(FFSRun::<
                    QuadTreeState<CanvasTube, NullStateTracker>,
                >::create_from_tileset::<KTAM>(
                    self, config
                )?)),
            },
            Model::ATAM => Err(GrowError::FFSCannotRunATAM.into()),
            Model::OldKTAM => match self.canvas_type.unwrap_or(CanvasType::Periodic) {
                CanvasType::Square => Ok(Box::new(FFSRun::<
                    QuadTreeState<CanvasSquare, NullStateTracker>,
                >::create_from_tileset::<OldKTAM>(
                    self, config
                )?)),
                CanvasType::Periodic => Ok(Box::new(FFSRun::<
                    QuadTreeState<CanvasPeriodic, NullStateTracker>,
                >::create_from_tileset::<OldKTAM>(
                    self, config
                )?)),
                CanvasType::Tube => Ok(Box::new(FFSRun::<
                    QuadTreeState<CanvasTube, NullStateTracker>,
                >::create_from_tileset::<OldKTAM>(
                    self, config
                )?)),
            },
        }
    }
}

pub struct FFSRun<St: State + StateTracked<NullStateTracker>> {
    pub level_list: Vec<FFSLevel<St>>,
    pub dimerization_rate: f64,
    pub forward_prob: Vec<f64>,
}

impl<St: State + StateTracked<NullStateTracker>> FFSResult for FFSRun<St> {
    fn nucleation_rate(&self) -> Rate {
        self.dimerization_rate * self.forward_prob.iter().fold(1., |acc, level| acc * *level)
    }

    fn forward_vec(&self) -> &Vec<f64> {
        &self.forward_prob
    }

    fn surfaces(&self) -> Vec<&dyn FFSSurface> {
        self.level_list
            .iter()
            .map(|level| level as &dyn FFSSurface)
            .collect()
    }

    fn dimerization_rate(&self) -> f64 {
        self.dimerization_rate
    }
}

impl<St: State + StateWithCreate<Params = (usize, usize)> + StateTracked<NullStateTracker>>
    FFSRun<St>
{
    pub fn create<Sy: SystemWithDimers + System>(
        system: &mut Sy,
        config: &FFSRunConfig,
    ) -> Result<Self, GrowError> {
        let level_list = Vec::new();

        let dimerization_rate = system
            .calc_dimers()
            .iter()
            .fold(0., |acc, d| acc + d.formation_rate);

        let mut ret = Self {
            level_list,
            dimerization_rate,
            forward_prob: Vec::new(),
        };

        let (first_level, dimer_level) = FFSLevel::nmers_from_dimers(system, config)?;

        ret.forward_prob.push(first_level.p_r);

        let mut current_size = first_level.target_size;

        ret.level_list.push(dimer_level);
        ret.level_list.push(first_level);

        let mut above_cutoff: usize = 0;

        while current_size < config.target_size {
            let last = ret.level_list.last_mut().unwrap();

            let next = last.next_level(system, config)?;
            if !config.keep_configs {
                last.drop_states();
            }
            let pf = next.p_r;
            ret.forward_prob.push(pf);
            // println!(
            //     "Done with target size {}: p_f {}, used {} trials for {} states.",
            //     last.target_size, pf, next.num_trials, next.num_states
            // );
            current_size = next.target_size;
            ret.level_list.push(next);

            if config.early_cutoff {
                if pf > config.cutoff_probability {
                    above_cutoff += 1;
                    if (above_cutoff > config.cutoff_number)
                        & (current_size >= config.min_cutoff_size)
                    {
                        break;
                    }
                } else {
                    above_cutoff = 0;
                }
            }

            if let Some(min_nuc_rate) = config.min_nuc_rate {
                if ret.nucleation_rate() < min_nuc_rate {
                    break;
                }
            }
        }

        Ok(ret)
    }
    pub fn dimer_conc(&self) -> f64 {
        self.level_list[0].p_r
    }
}

impl<St: State + StateWithCreate<Params = (usize, usize)> + StateTracked<NullStateTracker>>
    FFSRun<St>
{
    pub fn create_from_tileset<Sy: SystemWithDimers + System + FromTileSet>(
        tileset: &TileSet,
        config: &FFSRunConfig,
    ) -> Result<Self, RgrowError> {
        let mut sys = Sy::from_tileset(tileset)?;
        let c = {
            let mut c = config.clone();
            c.canvas_size = match tileset.size.unwrap_or(SIZE_DEFAULT) {
                tileset::Size::Single(x) => (x, x),
                tileset::Size::Pair(p) => p,
            };
            c
        };

        Ok(Self::create(&mut sys, &c)?)
    }
}

pub struct FFSLevel<St: State + StateTracked<NullStateTracker>> {
    pub state_list: Vec<St>,
    pub previous_list: Vec<usize>,
    pub p_r: f64,
    pub num_states: usize,
    pub num_trials: usize,
    pub target_size: NumTiles,
}

impl<St: State + StateTracked<NullStateTracker>> FFSSurface for FFSLevel<St> {
    fn get_config(&self, i: usize) -> ArrayView2<Tile> {
        self.state_list[i].raw_array()
    }

    fn num_configs(&self) -> usize {
        self.state_list.len()
    }

    fn target_size(&self) -> NumTiles {
        self.target_size
    }

    fn num_trials(&self) -> usize {
        self.num_trials
    }

    fn previous_list(&self) -> Vec<usize> {
        self.previous_list.clone()
    }
}

impl<St: State + StateWithCreate<Params = (usize, usize)> + StateTracked<NullStateTracker>>
    FFSLevel<St>
{
    pub fn drop_states(&mut self) -> &Self {
        self.state_list.drain(..);
        self
    }

    pub fn next_level<Sy: SystemWithDimers + System>(
        &self,
        system: &mut Sy,
        config: &FFSRunConfig,
    ) -> Result<Self, GrowError> {
        let mut rng = thread_rng();

        let mut state_list = Vec::new();
        let mut previous_list = Vec::new();
        let mut i = 0usize;
        let target_size = self.target_size + config.size_step;

        let bounds = {
            let mut b = config.subseq_bound;
            b.size_max = Some(target_size);
            b.size_min = Some(0);
            b
        };

        let chooser = Uniform::new(0, self.state_list.len());

        let canvas_size = self.state_list[0].get_params();

        let cvar = if config.constant_variance {
            config.var_per_mean2
        } else {
            0.
        };

        while state_list.len() < config.max_configs {
            let mut state = St::empty(canvas_size)?;

            let mut i_old_state: usize = 0;

            while state.n_tiles() == 0 {
                if state.total_rate() != 0. {
                    panic!("Total rate is not zero! {state:?}");
                };
                i_old_state = chooser.sample(&mut rng);

                state.zeroed_copy_from_state_nonzero_rate(&self.state_list[i_old_state]);
                debug_assert_eq!(system.calc_n_tiles(&state), state.n_tiles());

                system.evolve(&mut state, bounds).unwrap();
                i += 1;
            }

            if state.n_tiles() >= target_size {
                // >= hack for duples
                state_list.push(state);
                previous_list.push(i_old_state);
            } else {
                println!(
                    "Ran out of events: {} tiles, {} events, {} time, {} total rate.",
                    state.n_tiles(),
                    state.total_events(),
                    state.time(),
                    state.total_rate(),
                );
            }

            if (variance_over_mean2(state_list.len(), i) < cvar)
                & (state_list.len() >= config.min_configs)
            {
                break;
            }
        }
        let p_r = (state_list.len() as f64) / (i as f64);
        let num_states = state_list.len();

        Ok(Self {
            state_list,
            previous_list,
            p_r,
            target_size,
            num_states,
            num_trials: i,
        })
    }

    pub fn nmers_from_dimers<Sy: SystemWithDimers + System>(
        system: &mut Sy,
        config: &FFSRunConfig,
    ) -> Result<(Self, Self), GrowError> {
        let mut rng = SmallRng::from_entropy();

        let dimers = system.calc_dimers();

        let mut state_list = Vec::with_capacity(config.min_configs);
        let mut previous_list = Vec::with_capacity(config.min_configs);
        let mut i = 0usize;

        let mut dimer_state_list = Vec::with_capacity(config.min_configs);

        let weights: Vec<_> = dimers.iter().map(|d| d.formation_rate).collect();
        let chooser = WeightedIndex::new(weights).unwrap();

        if config.canvas_size.0 < 4 || config.canvas_size.1 < 4 {
            panic!("Canvas size too small for dimers");
        }
        let mid = PointSafe2((config.canvas_size.0 / 2, config.canvas_size.1 / 2));

        let mut num_states = 0usize;

        let mut tile_list = Vec::with_capacity(config.min_configs);

        let mut other: (usize, usize);

        let cvar = if config.constant_variance {
            config.var_per_mean2
        } else {
            0.
        };

        let bounds = {
            let mut b = config.subseq_bound;
            b.size_max = Some(config.start_size);
            b.size_min = Some(0);
            b
        };

        while state_list.len() < config.max_configs {
            let mut state = St::empty(config.canvas_size)?;

            while state.n_tiles() == 0 {
                let i_old_state = chooser.sample(&mut rng);
                let dimer = &dimers[i_old_state];

                other = match dimer.orientation {
                    Orientation::NS => state.move_sa_s(mid).0,
                    Orientation::WE => state.move_sa_e(mid).0,
                };
                system.set_points(&mut state, &[(mid.0, dimer.t1), (other, dimer.t2)]);

                debug_assert_eq!(system.calc_n_tiles(&state), state.n_tiles());

                system.evolve(&mut state, bounds).unwrap();
                i += 1;

                if state.n_tiles() >= config.start_size {
                    // FIXME: >= is a hack
                    // Create (retrospectively) a dimer state
                    let mut dimer_state = St::empty(config.canvas_size)?;
                    other = match dimer.orientation {
                        Orientation::NS => dimer_state.move_sa_s(mid).0,
                        Orientation::WE => dimer_state.move_sa_e(mid).0,
                    };
                    system.set_points(&mut dimer_state, &[(mid.0, dimer.t1), (other, dimer.t2)]);

                    state_list.push(state);

                    dimer_state_list.push(dimer_state);

                    if rng.gen::<bool>() {
                        tile_list.push(dimer.t1);
                    } else {
                        tile_list.push(dimer.t2);
                    }

                    previous_list.push(num_states);

                    num_states += 1;

                    break;
                } else {
                    if state.n_tiles() != 0 {
                        panic!("{}", state.panicinfo())
                    }
                    if state.total_rate() != 0. {
                        panic!("{}", state.panicinfo())
                    };
                }
            }

            if (variance_over_mean2(num_states, i) < cvar) & (num_states >= config.min_configs) {
                break;
            }
        }

        let p_r = (num_states as f64) / (i as f64);

        Ok((
            Self {
                state_list,
                previous_list,
                p_r,
                target_size: config.start_size,
                num_states,
                num_trials: i,
            },
            Self {
                state_list: dimer_state_list,
                previous_list: tile_list.into_iter().map(|x| x as usize).collect(),
                p_r: 1.0,
                target_size: 2,
                num_states,
                num_trials: num_states,
            },
        ))
    }
}

fn variance_over_mean2(num_success: usize, num_trials: usize) -> f64 {
    let ns = num_success as f64;
    let nt = num_trials as f64;
    let p = ns / nt;
    (1. - p) / (ns)
}

#[cfg_attr(feature = "python", pyclass(name = "FFSResult"))]
#[allow(dead_code)] // This is used in the python interface
pub struct BoxedFFSResult(pub(crate) Arc<Box<dyn ffs::FFSResult>>);

#[cfg(feature = "python")]
#[pymethods]
impl BoxedFFSResult {
    /// Nucleation rate, in M/s.  Calculated from the forward probability vector,
    /// and dimerization rate.
    #[getter]
    fn get_nucleation_rate(&self) -> f64 {
        self.0.nucleation_rate()
    }

    #[getter]
    fn get_forward_vec(&self) -> Vec<f64> {
        self.0.forward_vec().clone()
    }

    #[getter]
    fn get_dimerization_rate(&self) -> f64 {
        self.0.dimerization_rate()
    }

    #[getter]
    fn get_surfaces(&self) -> Vec<FFSLevelRef> {
        self.0
            .surfaces()
            .iter()
            .enumerate()
            .map(|(i, _)| FFSLevelRef {
                res: self.0.clone(),
                level: i,
            })
            .collect()
    }

    fn __str__(&self) -> String {
        format!(
            "FFSResult({:1.4e} M/s, {:?})",
            self.0.nucleation_rate(),
            self.0.forward_vec()
        )
    }

    fn __repr__(&self) -> String {
        self.__str__()
    }

    #[getter]
    fn previous_indices(&self) -> Vec<Vec<usize>> {
        self.get_surfaces()
            .iter()
            .map(|x| x.get_previous_indices())
            .collect()
    }
}

#[cfg_attr(feature = "python", pyclass)]
#[allow(dead_code)] // This is used in the python interface
pub struct FFSLevelRef {
    res: Arc<Box<dyn ffs::FFSResult>>,
    level: usize,
}

#[cfg(feature = "python")]
#[pymethods]
impl FFSLevelRef {
    #[getter]
    fn get_configs<'py>(&self, py: Python<'py>) -> Vec<&'py PyArray2<crate::base::Tile>> {
        self.res.surfaces()[self.level]
            .configs()
            .iter()
            .map(|x| x.to_pyarray(py))
            .collect()
    }

    #[getter]
    fn get_previous_indices(&self) -> Vec<usize> {
        self.res.surfaces()[self.level].previous_list()
    }
}
