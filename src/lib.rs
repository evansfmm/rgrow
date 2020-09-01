extern crate ndarray;
use fnv::FnvHashSet;
use std::marker::PhantomData;
extern crate num_traits;
use ndarray::prelude::*;
use ndarray::{FoldWhile, Zip};
use rand::prelude::*;
use std::convert::TryInto;
pub type NumTiles = usize;
pub type NumEvents = u64;
pub type Point = (usize, usize);
pub type Tile = usize;
pub type Rate = f64;
pub type Energy = f64;

pub trait StateEvolve<C: Canvas, S: System<C>> {
    fn evolve_in_size_range(
        &mut self,
        system: &S,
        minsize: NumTiles,
        maxsize: NumTiles,
        maxevents: NumEvents,
    ) -> &Self;
}

pub trait StateStep<C: Canvas, S: System<C>> {
    fn take_step(&mut self, system: &S) -> &Self;
}

pub trait StateCreate<C: Canvas, S: System<C>> {
    fn create(canvas: &Array2<Tile>, sys: &S) -> Self;
}

pub trait StateStatus {
    fn ntiles(&self) -> NumTiles;

    fn total_events(&self) -> NumEvents;

    //fn time(&self) -> Time;

    //fn last_step_time(&self) -> Time;
}

pub trait Canvas {
    unsafe fn uv_n(&self, p: Point) -> Tile;
    unsafe fn uv_e(&self, p: Point) -> Tile;
    unsafe fn uv_s(&self, p: Point) -> Tile;
    unsafe fn uv_w(&self, p: Point) -> Tile;
    unsafe fn uv_p(&self, p: Point) -> Tile;
    fn inbounds(&self, p: Point) -> bool;
}
pub trait System<C: Canvas> {
    fn event_rate_at_point(&self, canvas: &C, p: Point) -> Rate;

    fn choose_event_at_point(&self, canvas: &C, p: Point, acc: Rate) -> Tile;
}

#[derive(Clone)]
pub struct StaticATAM {
    tile_rates: Array1<Rate>,
    strength_ns: Array2<Energy>,
    strength_we: Array2<Energy>,
    tau: Energy,
}

#[derive(Clone)]
pub struct StaticKTAM {
    tile_rates: Array1<Rate>,
    energy_ns: Array2<Energy>,
    energy_we: Array2<Energy>,
    friends_n: Vec<FnvHashSet<Tile>>,
    friends_e: Vec<FnvHashSet<Tile>>,
    friends_s: Vec<FnvHashSet<Tile>>,
    friends_w: Vec<FnvHashSet<Tile>>
}

#[derive(Clone)]
pub struct State2DQT<S: System<Canvas2D>> {
    pub rates: Vec<Array2<Rate>>,
    pub canvas: Canvas2D,
    phantomsys: PhantomData<S>,
    ntiles: NumTiles,
    total_rate: Rate,
    total_events: NumEvents,
    //time: f64
}

#[derive(Clone)]
pub struct Canvas2D {
    pub canvas: Array2<Tile>,
    size: usize,
}

impl StaticATAM {
    pub fn new(
        tile_concs: Array1<f64>,
        tile_edges: Array2<usize>,
        glue_strengths: Array1<f64>,
        tau: f64,
    ) -> Self {
        let ntiles = tile_concs.len();
        assert!(ntiles == tile_edges.nrows());
        let mut strength_we: Array2<Energy> = Array2::zeros((ntiles, ntiles));
        let mut strength_ns: Array2<Energy> = Array2::zeros((ntiles, ntiles));
        for ti1 in 0..ntiles {
            for ti2 in 0..ntiles {
                let t1 = tile_edges.row(ti1);
                let t2 = tile_edges.row(ti2);
                if t1[2] == t2[0] {
                    strength_ns[(ti1, ti2)] = glue_strengths[t1[2]];
                }
                if t1[1] == t2[3] {
                    strength_we[(ti1, ti2)] = glue_strengths[t1[1]];
                }
            }
        }
        return StaticATAM {
            tile_rates: tile_concs,
            strength_ns,
            strength_we,
            tau,
        };
    }
}

fn create_friend_data(
    energy_ns: &Array2<Energy>,
    energy_we: &Array2<Energy>,
) -> (
    Vec<FnvHashSet<Tile>>,
    Vec<FnvHashSet<Tile>>,
    Vec<FnvHashSet<Tile>>,
    Vec<FnvHashSet<Tile>>,
) {
    let mut friends_n = Vec::<FnvHashSet<Tile>>::new();
    let mut friends_e = Vec::<FnvHashSet<Tile>>::new();
    let mut friends_s = Vec::<FnvHashSet<Tile>>::new();
    let mut friends_w = Vec::<FnvHashSet<Tile>>::new();
    
    for _t1 in 0..energy_ns.nrows() {
        friends_n.push(FnvHashSet::default());
        friends_e.push(FnvHashSet::default());
        friends_s.push(FnvHashSet::default());
        friends_w.push(FnvHashSet::default());
    }

    for t1 in 0..energy_ns.nrows() {
        for t2 in 0..energy_ns.nrows() {
            if energy_ns[(t1, t2)] != 0. {
                friends_s[t1].insert(t2);
                friends_n[t2].insert(t1);
            }
            if energy_we[(t1, t2)] != 0. {
                friends_e[t1].insert(t2);
                friends_w[t2].insert(t1);
            }
        }
    };

    (friends_n, friends_e, friends_s, friends_w)
}

impl StaticKTAM {
    pub fn new(
        tile_concs: Array1<f64>,
        tile_edges: Array2<usize>,
        glue_strengths: Array1<f64>,
        gse: f64,
    ) -> Self {
        let ntiles = tile_concs.len();
        assert!(ntiles == tile_edges.nrows());
        let mut energy_we: Array2<Energy> = Array2::zeros((ntiles, ntiles));
        let mut energy_ns: Array2<Energy> = Array2::zeros((ntiles, ntiles));
        for ti1 in 0..ntiles {
            for ti2 in 0..ntiles {
                let t1 = tile_edges.row(ti1);
                let t2 = tile_edges.row(ti2);
                if t1[2] == t2[0] {
                    energy_ns[(ti1, ti2)] = gse * glue_strengths[t1[2]];
                }
                if t1[1] == t2[3] {
                    energy_we[(ti1, ti2)] = gse * glue_strengths[t1[1]];
                }
            }
        }
        let (friends_n, friends_e, friends_s, friends_w) = create_friend_data(&energy_ns, &energy_we);
        return StaticKTAM {
            tile_rates: tile_concs,
            energy_ns,
            energy_we,
            friends_n,
            friends_e,
            friends_s,
            friends_w,
        };
    }

    pub fn from_raw(
        tile_rates: Array1<f64>,
        energy_ns: Array2<Energy>,
        energy_we: Array2<Energy>,
    ) -> Self {
        let (friends_n, friends_e, friends_s, friends_w) = create_friend_data(&energy_ns, &energy_we);

        StaticKTAM {
            tile_rates,
            energy_ns,
            energy_we,
            friends_n,
            friends_e,
            friends_s,
            friends_w,
        }
    }
}

impl<S> StateCreate<Canvas2D, S> for State2DQT<S>
where
    S: System<Canvas2D> + Clone,
{
    fn create(canvas: &Array2<Tile>, sys: &S) -> Self {
        assert!(canvas.nrows().is_power_of_two());

        let p: u32 = (1 + canvas.nrows().trailing_zeros()).try_into().unwrap();

        let mut rates = Vec::<Array2<Rate>>::new();

        for i in (1..p).rev() {
            rates.push(Array2::<Rate>::zeros((2usize.pow(i), 2usize.pow(i))))
        }

        let size = canvas.nrows();

        let ncanvas = Canvas2D {
            canvas: canvas.to_owned(),
            size,
        };

        let mut ret = State2DQT::<S> {
            rates: rates,
            canvas: ncanvas,
            phantomsys: PhantomData,
            ntiles: canvas.fold(0, |x, y| x + (if *y == 0 { 0 } else { 1 })),
            total_rate: 0.,
            total_events: 0,
        };

        for y in 1..size - 1 {
            for x in 1..size - 1 {
                // FIXME: not at all ideal
                ret.update_rates_single_noprop(sys, (y, x));
            }
        }
        ret.rebuild_ratetree();

        ret
    }
}

impl<C> System<C> for StaticATAM
where
    C: Canvas,
{
    fn event_rate_at_point(&self, canvas: &C, p: Point) -> Rate {
        if !canvas.inbounds(p) {
            return 0.0;
        }

        // Bound is previously checked.
        let tile = unsafe { canvas.uv_p(p) };

        let tn = unsafe { canvas.uv_n(p) };
        let te = unsafe { canvas.uv_e(p) };
        let ts = unsafe { canvas.uv_s(p) };
        let tw = unsafe { canvas.uv_w(p) };

        if tile != 0 {
            // Deletion
            0.0
        } else {
            // Insertion

            Zip::from(self.strength_ns.row(tn))
                .and(self.strength_we.column(te))
                .and(self.strength_we.row(tw))
                .and(self.strength_ns.column(ts))
                .and(&self.tile_rates)
                .fold(0., |acc, &n, &e, &s, &w, &r| {
                    if n + e + s + w >= self.tau {
                        acc + r
                    } else {
                        acc
                    }
                })
        }
    }

    fn choose_event_at_point(&self, canvas: &C, p: Point, acc: Rate) -> Tile {
        if !canvas.inbounds(p) {
            panic!("Oh dear!");
        }

        // Bound is previously checked.
        let tile = unsafe { canvas.uv_p(p) };

        if tile != 0 {
            // Deletion is easy!
            panic!("We shouldn't be deleting in the aTAM!")
        } else {
            let tn = unsafe { canvas.uv_n(p) };
            let te = unsafe { canvas.uv_e(p) };
            let ts = unsafe { canvas.uv_s(p) };
            let tw = unsafe { canvas.uv_w(p) };

            // Insertion is hard!
            let r = Zip::indexed(self.strength_ns.row(tn))
                .and(self.strength_we.column(te))
                .and(self.strength_ns.column(ts))
                .and(self.strength_we.row(tw))
                .and(&self.tile_rates)
                .fold_while((acc, 0), |(acc, _v), i, &n, &e, &s, &w, &r| {
                    if n + e + s + w >= self.tau {
                        if acc - r > 0. {
                            FoldWhile::Continue((acc - r, 0))
                        } else {
                            FoldWhile::Done((acc - r, i))
                        }
                    } else {
                        FoldWhile::Continue((acc, 0))
                    }
                });

            match r {
                FoldWhile::Done((_acc, i)) => i,

                FoldWhile::Continue((_acc, _i)) => panic!(),
            }
        }
    }
}

impl<C> System<C> for StaticKTAM
where
    C: Canvas,
{
    fn event_rate_at_point(&self, canvas: &C, p: Point) -> Rate {
        if !canvas.inbounds(p) {
            return 0.0;
        }

        // Bound is previously checked.
        let tile = unsafe { canvas.uv_p(p) };

        let tn = unsafe { canvas.uv_n(p) };
        let te = unsafe { canvas.uv_e(p) };
        let ts = unsafe { canvas.uv_s(p) };
        let tw = unsafe { canvas.uv_w(p) };

        if tile != 0 {
            // Deletion

            // Bound is previously checked
            let bound_energy: Energy = self.energy_ns[(tile, ts)]
                + self.energy_ns[(tn, tile)]
                + self.energy_we[(tile, te)]
                + self.energy_we[(tw, tile)];

            Rate::exp(-bound_energy)
        } else if (tn == 0) & (te == 0) & (tw == 0) & (ts == 0) {
            // Short circuit for no possibility of insertion (no adjacents)
            0.0
        } else {
            // Insertion

            let mut friends = FnvHashSet::<Tile>::default();

            if tn != 0 {friends.extend(&self.friends_s[tn]);}
            if te != 0 {friends.extend(&self.friends_w[te]);}
            if ts != 0 {friends.extend(&self.friends_n[ts]);}
            if tw != 0 {friends.extend(&self.friends_e[tw]);}

            let mut acc = 0.;
            for t in friends.drain() {
                acc += self.tile_rates[t];
            }

            acc

        //     Zip::from(self.energy_ns.row(tn))
        //         .and(self.energy_we.column(te))
        //         .and(self.energy_ns.column(ts))
        //         .and(self.energy_we.row(tw))
        //         .and(&self.tile_rates)
        //         .fold(0., |acc, &n, &e, &s, &w, &r| {
        //             if (n != 0.) | (e != 0.) | (s != 0.) | (w != 0.) {
        //                 acc + r
        //             } else {
        //                 acc
        //             }
        //         })
        }
    }

    fn choose_event_at_point(&self, canvas: &C, p: Point, mut acc: Rate) -> Tile {
        if !canvas.inbounds(p) {
            panic!("Oh dear!");
        }

        // Bound is previously checked.
        let tile = unsafe { canvas.uv_p(p) };

        if tile != 0 {
            // Deletion is easy!
            0
        } else {
            let tn = unsafe { canvas.uv_n(p) };
            let te = unsafe { canvas.uv_e(p) };
            let ts = unsafe { canvas.uv_s(p) };
            let tw = unsafe { canvas.uv_w(p) };

            let mut friends = FnvHashSet::<Tile>::default();

            friends.extend(&self.friends_s[tn]);
            friends.extend(&self.friends_w[te]);
            friends.extend(&self.friends_n[ts]);
            friends.extend(&self.friends_e[tw]);

            for t in friends.drain() {
                acc -= self.tile_rates[t];
                if acc <= 0. {return t};
            }

            panic!();

            // // Insertion is hard!
            // let r = Zip::indexed(self.energy_ns.row(tn))
            //     .and(self.energy_we.column(te))
            //     .and(self.energy_ns.column(ts))
            //     .and(self.energy_we.row(tw))
            //     .and(&self.tile_rates)
            //     .fold_while((acc, 0), |(acc, _v), i, &n, &e, &s, &w, &r| {
            //         if (n != 0.) | (e != 0.) | (s != 0.) | (w != 0.) {
            //             if acc - r > 0. {
            //                 FoldWhile::Continue((acc - r, 0))
            //             } else {
            //                 FoldWhile::Done((acc - r, i))
            //             }
            //         } else {
            //             FoldWhile::Continue((acc, 0))
            //         }
            //     });

            // match r {
            //     FoldWhile::Done((_acc, i)) => i,

            //     FoldWhile::Continue((_acc, _i)) => panic!(),
            // }
        }
    }
}

impl<S> StateEvolve<Canvas2D, S> for State2DQT<S>
where
    S: System<Canvas2D>,
{
    fn evolve_in_size_range(
        &mut self,
        system: &S,
        minsize: NumTiles,
        maxsize: NumTiles,
        maxevents: NumEvents,
    ) -> &Self {
        let mut events: NumEvents = 0;

        while events < maxevents {
            self.take_step(system);

            if (self.ntiles() <= minsize) | (self.ntiles() >= maxsize) {
                return self;
            }

            events += 1;
        }

        panic!("Too many events!");
    }
}

impl Canvas for Canvas2D {
    #[inline]
    unsafe fn uv_n(&self, p: Point) -> Tile {
        *self.canvas.uget((p.0 - 1, p.1))
    }

    #[inline]
    unsafe fn uv_e(&self, p: Point) -> Tile {
        *self.canvas.uget((p.0, p.1 + 1))
    }

    #[inline]
    unsafe fn uv_s(&self, p: Point) -> Tile {
        *self.canvas.uget((p.0 + 1, p.1))
    }

    #[inline]
    unsafe fn uv_w(&self, p: Point) -> Tile {
        *self.canvas.uget((p.0, p.1 - 1))
    }

    #[inline]
    unsafe fn uv_p(&self, p: Point) -> Tile {
        *self.canvas.uget(p)
    }

    #[inline]
    fn inbounds(&self, p: Point) -> bool {
        return (p.0 >= 1) & (p.1 >= 1) & (p.0 < self.size - 1) & (p.1 < self.size - 1);
    }
}

#[macro_use]
macro_rules! plusar {
    ( $p:expr  ) => {
        &[
            $p,
            ($p.0 - 1, $p.1),
            ($p.0 + 1, $p.1),
            ($p.0, $p.1 - 1),
            ($p.0, $p.1 + 1),
        ]
    };
}

impl<S> State2DQT<S>
where
    S: System<Canvas2D>,
{
    fn choose_event_point(&self) -> (Point, Rate) {
        let mut threshold = self.total_rate * random::<Rate>();

        let mut x: usize = 0;
        let mut y: usize = 0;

        for r in self.rates.iter().rev() {
            y *= 2;
            x *= 2;
            if threshold - r[(y, x)] <= 0. {
                continue;
            } else {
                threshold -= r[(y, x)];
                x += 1;
            }
            if threshold - r[(y, x)] <= 0. {
                continue;
            } else {
                threshold -= r[(y, x)];
                y += 1;
                x -= 1
            }
            if threshold - r[(y, x)] <= 0. {
                continue;
            } else {
                threshold -= r[(y, x)];
                x += 1;
            }
            if threshold - r[(y, x)] <= 0. {
                continue;
            } else {
                panic!();
            }
        }
        return ((y, x), threshold);
    }

    pub fn create_we_pair(sys: &S, w: Tile, e: Tile, size: usize) -> Self {
        assert!(size.is_power_of_two());
        assert!(size > 8);

        let mut canvas = Array2::<Tile>::zeros((size, size));

        let p: u32 = (1 + size.trailing_zeros()).try_into().unwrap();

        let mut rates = Vec::<Array2<Rate>>::new();

        for i in (1..p).rev() {
            rates.push(Array2::<Rate>::zeros((2usize.pow(i), 2usize.pow(i))))
        }

        let size = canvas.nrows();

        canvas[(size / 2, size / 2)] = w;
        canvas[(size / 2, size / 2 + 1)] = e;

        let ncanvas = Canvas2D {
            canvas: canvas,
            size,
        };

        let mut ret = State2DQT::<S> {
            rates: rates,
            canvas: ncanvas,
            phantomsys: PhantomData,
            ntiles: 2,
            total_rate: 0.,
            total_events: 0,
        };

        ret.update_rates_ps(sys, (size / 2, size / 2));
        ret.update_rates_ps(sys, (size / 2, size / 2 + 1));

        ret
    }

    pub fn create_ns_pair(sys: &S, n: Tile, s: Tile, size: usize) -> Self {
        assert!(size.is_power_of_two());
        assert!(size > 8);

        let mut canvas = Array2::<Tile>::zeros((size, size));

        let p: u32 = (1 + size.trailing_zeros()).try_into().unwrap();

        let mut rates = Vec::<Array2<Rate>>::new();

        for i in (1..p).rev() {
            rates.push(Array2::<Rate>::zeros((2usize.pow(i), 2usize.pow(i))))
        }

        let size = canvas.nrows();

        canvas[(size / 2, size / 2)] = n;
        canvas[(size / 2 + 1, size / 2)] = s;

        let ncanvas = Canvas2D {
            canvas: canvas,
            size,
        };

        let mut ret = State2DQT::<S> {
            rates: rates,
            canvas: ncanvas,
            phantomsys: PhantomData,
            ntiles: 2,
            total_rate: 0.,
            total_events: 0,
        };

        ret.update_rates_ps(sys, (size / 2, size / 2));
        ret.update_rates_ps(sys, (size / 2 + 1, size / 2));

        ret
    }

    pub fn set_point(&mut self, sys: &S, p: Point, t: Tile) -> &Self {
        let ot = self.canvas.canvas[p];
        self.canvas.canvas[p] = t;
        self.update_rates_ps(sys, p);
        if (t == 0) & (ot != 0) {
            self.ntiles -= 1
        } else if (t != 0) & (ot == 0) {
            self.ntiles += 1
        };

        self
    }

    fn do_event_at_location(&mut self, system: &S, p: Point, acc: Rate) -> &Self {
        let newtile = system.choose_event_at_point(&self.canvas, p, acc);

        if newtile == 0 {
            self.ntiles -= 1
        } else {
            self.ntiles += 1
        };

        self.total_events += 1;

        // Repeatedly checked!
        unsafe { *self.canvas.canvas.uget_mut(p) = newtile };

        self.update_rates_ps(system, p)
    }

    fn update_rates_ps(&mut self, system: &S, p: Point) -> &Self {
        let mut rtiter = self.rates.iter_mut();

        // The base level
        let mut rt = rtiter.next().unwrap();
        let mut np: (usize, usize) = p.clone();

        for ps in plusar!(p) {
            rt[*ps] = system.event_rate_at_point(&self.canvas, *ps);
        }

        let mut div: usize = 2;

        for rn in rtiter {
            np = (np.0 / 2, np.1 / 2);

            qt_update_level(rn, rt, np);

            // If on boundary of , update to N; if on
            if p.0 % div == 0 {
                qt_update_level(rn, rt, (np.0 - 1, np.1))
            } else if (p.0 + 1) % div == 0 {
                qt_update_level(rn, rt, (np.0 + 1, np.1))
            };

            if p.1 % div == 0 {
                qt_update_level(rn, rt, (np.0, np.1 - 1))
            } else if (p.1 + 1) % div == 0 {
                qt_update_level(rn, rt, (np.0, np.1 + 1))
            };

            div *= 2;

            rt = rn;
        }

        self.total_rate = rt.sum();

        return self;
    }

    fn update_rates_single(&mut self, system: &S, p: Point) -> &Self {
        let mut rtiter = self.rates.iter_mut();
        let mut rt = rtiter.next().unwrap();
        let mut np: (usize, usize) = p.clone();

        rt[p] = system.event_rate_at_point(&self.canvas, p);

        for rn in rtiter {
            np = (np.0 / 2, np.1 / 2);
            qt_update_level(rn, rt, np);
            rt = rn;
        }

        self.total_rate = rt.sum();

        return self;
    }

    fn update_rates_single_noprop(&mut self, system: &S, p: Point) -> &Self {
        let mut rt = self.rates.iter_mut().next().unwrap();

        rt[p] = system.event_rate_at_point(&self.canvas, p);

        return self;
    }

    fn rebuild_ratetree(&mut self) -> &Self {
        let mut rtiter = self.rates.iter_mut();
        let mut rt = rtiter.next().unwrap();

        for rn in rtiter {
            for (p, v) in rn.indexed_iter_mut() {
                qt_update_level_val(v, rt, p)
            }

            rt = rn;
        }

        self.total_rate = rt.sum();

        self
    }
}

#[inline]
fn qt_update_level(rn: &mut Array2<Rate>, rt: &Array2<Rate>, np: Point) {
    let ip = (np.0 * 2, np.1 * 2);

    unsafe {
        *rn.uget_mut(np) = *rt.uget(ip)
            + *rt.uget((ip.0, ip.1 + 1))
            + *rt.uget((ip.0 + 1, ip.1))
            + *rt.uget((ip.0 + 1, ip.1 + 1));
    }
}

#[inline]
fn qt_update_level_val(rn: &mut f64, rt: &Array2<Rate>, np: Point) {
    let ip = (np.0 * 2, np.1 * 2);

    unsafe {
        *rn = *rt.uget(ip)
            + *rt.uget((ip.0, ip.1 + 1))
            + *rt.uget((ip.0 + 1, ip.1))
            + *rt.uget((ip.0 + 1, ip.1 + 1));
    }
}

impl<S> StateStep<Canvas2D, S> for State2DQT<S>
where
    S: System<Canvas2D>,
{
    fn take_step(&mut self, system: &S) -> &Self {
        // Decide on a point.
        let (p, acc) = self.choose_event_point();

        // Do the event.
        self.do_event_at_location(system, p, acc);

        // Update rates around the point (point, and NESW)
        // Only works without fission and without double tiles!
        self.update_rates_ps(system, p);

        return self;
    }
}

impl<S> StateStatus for State2DQT<S>
where
    S: System<Canvas2D>,
{
    fn ntiles(&self) -> NumTiles {
        self.ntiles
    }

    fn total_events(&self) -> NumEvents {
        self.total_events
    }
}
