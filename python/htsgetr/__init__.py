"""
htsgetr - htsget protocol server implementation in Rust with Python bindings

Usage:
    from htsgetr import HtsgetServer, HtsgetClient

    # Start a server
    server = HtsgetServer("/path/to/data")
    server.run()

    # Or use the client
    client = HtsgetClient("http://localhost:8080")
    result = client.reads("sample1", reference_name="chr1", start=0, end=1000000)
"""

from htsgetr._htsgetr import HtsgetServer, HtsgetClient

__version__ = "0.1.0"
__all__ = ["HtsgetServer", "HtsgetClient", "__version__"]
