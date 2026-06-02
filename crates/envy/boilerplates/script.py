"""
"""

from loguru import logger
import polars as pl

# Flag to toggle printing polars schema
verbose: bool = False

logger.remove()  # Clear default handler
logger.add(
    sys.stderr,
    level="INFO" if verbose else "WARNING",
    format="<green>{time:HH:mm:ss}</green> | <level>{level: <8}</level> | <level>{message}</level>",
)



