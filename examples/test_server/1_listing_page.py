from bs4 import BeautifulSoup


def navigate(content: str) -> list[(str, int)]:
    html = BeautifulSoup(content, 'html.parser')
    links = []
    for a in html.select("section.pager a"):
        link = a.attrs['href']
        links.append((link, 1))

    for a in html.select("ul a"):
        link = a.attrs['href']
        links.append((link, 2))
    return links