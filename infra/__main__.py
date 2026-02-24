"""Pulumi entry point for skreg infrastructure."""
import logging

import structlog

from skillpkg_infra.__main__ import SkillpkgStack
from skillpkg_infra.config import StackConfig

structlog.configure(wrapper_class=structlog.make_filtering_bound_logger(logging.INFO))
SkillpkgStack(config=StackConfig.load()).run()
