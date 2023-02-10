TYPE_ID: int = 1


def navigate(content: str) -> list[(str, int)]:
    """
    Returns list of page outgoing links (next pages for parsing) as well as their PAGE_TYPE_IDs:
    ```
    [
      ("/url1", 1),
      ("/url2", 2),
    ]
    """
    return []


def parse(content: str) -> dict[str, str]:
    """
    Returns parsed key-value pairs for a given page
    """
    return {}


def validate(content: str) -> bool:
    """
    Checks if page content is valid. If `False` page will be downloaded again
    """
    return True
