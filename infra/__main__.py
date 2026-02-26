"""Pulumi entry point for skreg infrastructure."""
import logging

import structlog

from skreg_infra.__main__ import SkregStack
from skreg_infra.config import StackConfig

structlog.configure(wrapper_class=structlog.make_filtering_bound_logger(logging.INFO))
SkregStack(config=StackConfig.load()).run()
