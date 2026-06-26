from risio import ImageF64
from pyMilk.interfacing.shm import SHM
import numpy as np
import time
import rich

WIDTH = 100
NITER = 1000

if __name__ == "__main__":
    a = ImageF64("a", [WIDTH, WIDTH])
    b = ImageF64("b", [WIDTH, WIDTH])
    a0 = 42.0
    a.write([a0]+list(np.random.randn(WIDTH*WIDTH-1)))
    b.write([0.0]*(WIDTH*WIDTH))
    t1 = time.perf_counter()
    for i in range(NITER):
        b.write([ai+bi for (ai, bi) in zip(a.read(),b.read())])
    t2 = time.perf_counter()
    rich.print(f"{(t2-t1)/NITER:0.3e} sec per iteration")
    print()
    v1 = b.read()[0]
    v2 = a0*(NITER)
    print(v1, v2)
    a.block()
    b.block()