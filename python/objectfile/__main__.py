"""Entry point for ``python -m objectfile``."""

from __future__ import annotations

from ._cli import main

if __name__ == "__main__":
    raise SystemExit(main())
