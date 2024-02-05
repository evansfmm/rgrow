from enum import Enum
from typing import TYPE_CHECKING
import numpy as np

if TYPE_CHECKING:  # pragma: no cover
    pass

class TileShape(Enum):
    Single = ...
    Vertical = ...
    Horizontal = ...

class EvolveOutcome(object): ...

class State(object):
    @property
    def canvas_view(self) -> np.ndarray: ...
    def canvas_copy(self) -> np.ndarray: ...
    @property
    def ntiles(self) -> int: ...
    @property
    def time(self) -> float: ...
    @property
    def total_events(self) -> int: ...

class System(object):
    def evolve(self, state, **kwargs): ...
    def evolve_states(self, states, **kwargs): ...
    def calc_mismatches(self, state) -> int: ...
    def name_canvas(self, state) -> np.ndarray: ...
    def update_all(self, state, needed) -> None: ...

class FissionHandling(object): ...
class CanvasType(object): ...
class ChunkSize(object): ...
class ChunkHandling(object): ...
class Model(object): ...

class FFSLevel(object):
    @property
    def configs(self) -> list[np.ndarray]:
        """List of configurations at this level, as arrays (not full states)."""
        ...
    @property
    def previous_indices(self) -> list[int]:
        """For each configuration, the index of the configuration in the previous
        level that resulted in it."""
        ...

class FFSResult(object):
    @property
    def nucleation_rate(self) -> float:
        """
        The calculated nucleation rate, in M/s.
        """
        ...
    @property
    def forward_vec(self) -> np.ndarray: ...
    @property
    def dimerization_rate(self) -> float: ...
    @property
    def surfaces(self) -> list[FFSLevel]: ...
    @property
    def previous_indices(self) -> list[list[int]]: ...

class Tile(object):
    def __init__(
        self,
        bonds: list[str | int],
        name: str | None = None,
        stoic: float | None = None,
        color: str | None = None,
    ) -> None: ...

class TileSet(object):
    def __init__(self, **kwargs) -> None: ...
    def create_system(self, **kwargs) -> System: ...
    def create_system_and_state(self, **kwargs) -> tuple[System, State]: ...
    def create_state(self, **kwargs) -> State: ...
    def create_state_empty(self, **kwargs) -> State: ...
    def run_window(self, **kwargs) -> tuple[System, State]: ...
    def run_ffs(self, **kwargs) -> FFSResult: ...

class EvolveBounds(object):
    def __init__(
        self,
        for_events: int | None = None,
        total_events: int | None = None,
        for_time: float | None = None,
        total_time: float | None = None,
        size_min: float | None = None,
        size_max: float | None = None,
        for_wall_time: float | None = None,
        require_strong_bound: bool = True,
    ) -> None: ...

class FFSRunConfig(object):
    # Use constant-variance, variable-configurations-per-surface method.
    # If false, use max_configs for each surface.
    @property
    def constant_variance(self) -> bool: ...
    # Variance per mean^2 for constant-variance method.
    @property
    def var_per_mean2(self) -> float: ...
    # Minimum number of configuratons to generate at each level.
    @property
    def min_configs(self) -> int: ...
    # Maximum number of configurations to generate at each level.
    @property
    def max_configs(self) -> int: ...
    # Use early cutoff for constant-variance method.
    @property
    def early_cutoff(self) -> bool: ...
    @property
    def cutoff_probability(self) -> float: ...
    @property
    def cutoff_number(self) -> int: ...
    @property
    def min_cutoff_size(self) -> int: ...
    @property
    def init_bound(self) -> EvolveBounds: ...
    @property
    def subseq_bound(self) -> EvolveBounds: ...
    @property
    def start_size(self) -> int: ...
    @property
    def size_step(self) -> int: ...
    @property
    def keep_configs(self) -> bool: ...
    @property
    def min_nuc_rate(self) -> float | None: ...
    @property
    def canvas_size(self) -> tuple[int, int]: ...
    @property
    def target_size(self) -> int: ...
