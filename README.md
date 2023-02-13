# Crab

Parser-toolkit for fast and furious crawling and parsing sites on the Internet. The main goal is to provide tools for implementing independent crawling and parsing.

## Why independent crawling matters?

Nearly all parsers are written using try and error approach. If crawling and parsing are done together you are forced to download documents from Internet each time you change a parser. Crab tries to separate crawling logic from parsing logic and maintain localy stored mirror of pages you are parsing.

## Architecture

```mermaid
flowchart LR
  navigator
  parser
  tabulator
  internet((Internet))
  validator

  subgraph db
    pages[(Pages)]
    links[(Links)]
  end

  subgraph rules
    parser
    navigator
    validator
  end

  csv[(CSV)]

  crawler --> pages
  internet --> crawler --> validator --> navigator --> links --> pages
  pages --> parser --> tabulator --> csv
```

## Installing

### Prerequirements

* rust toolchain
* python 3

At the moment no prebuilt binaries are available. To install crab you need to have rust toolchain installed

```
$ git clone https://github.com/bazhenov/crab
$ cd crab
$ cargo install --path=.
```

## Gettings started

### Creating Workspace

Let's write simple parser for brainyquote.com. Start a new project

```
$ crab new brainyquote
$ cd brainyquote
$ ls
db.sqlite
parser_home_page.py
```

Crab creates main database file as well as skeleton of python parser.

### Downloading first page

Now let's register our first page in a database

```
$ crab register https://www.brainyquote.com/ 1
```

Here we provide URL and a page type. Each downloaded and parsed page has it's own type id. Different page types are processed by different parsers. It allows to separate navigating and parsing logic of complex sites.

Now let's download first page using following command

```
$ crab run-crawler
```

This command will download all not already downloaded pages in the database. In our case we have only one page. We can confirm it's downloaded using following command

```
$ crab list-pages
     id  type_id  depth  status           url
------------------------------------------------------------------------------------------------------------------------
      1        1      0  downloaded       https://www.brainyquote.com/
```

This command allows to inspect state of all the pages in the database. We also can get the content of a page

```
$ crab dump 1

<!DOCTYPE html>
<html lang="en">
<head>
<title>Inspirational Quotes at BrainyQuote</title>
<meta name="robots" content="all">
<meta charset="utf-8" />
<meta name="viewport" content="width=device-width, initial-scale=1.0, viewport-fit=cover"><meta name="description" content="Share our collection of inspirational and famous quotes by authors you know and love. Share our Quotes of the Day on the web, Facebook, Twitter, and blogs.">
<meta name="googlebot" content="NOODP">
<meta property="ver" content="13.3.11:5238611">
...
```

### Writing parser

At this stage we have download one page, but crab has no information how to navigate between pages. Let's write simple rule how to navigate to the quotes page of a particular author.

Open `parser_home_page.py` and write following `navigate()` function:

```python
from bs4 import BeautifulSoup

def navigate(content: str) -> list[(str, int)]:
    html = BeautifulSoup(content, 'html.parser')
    links = []
    for a in html.select(".homeAuthorsExploreBlock a"):
        if a['href'] and '/authors/' in a['href']:
            links.append((a['href'], 2))
    return links
```

Here we're using [BeautifulSoup](https://www.crummy.com/software/BeautifulSoup/bs4/doc/) as a HTMLparsing toolkit. It's a simple rule that retrive all the links in an element with css class `homeAuthorsExploreBlock`. It checks if URL contains `/authors/` and return URL to crab as a next page for parsing.

Note that each url is tagged with number `2`. This is page type for a target page.

We can now check if out rules are works correctly:

```
$ crab navigate 1
  2  https://www.brainyquote.com/authors/a-p-j-abdul-kalam-quotes
  2  https://www.brainyquote.com/authors/alan-watts-quotes
  2  https://www.brainyquote.com/authors/albert-camus-quotes
  2  https://www.brainyquote.com/authors/alexander-the-great-quotes
  2  https://www.brainyquote.com/authors/amelia-earhart-quotes
  2  https://www.brainyquote.com/authors/anne-frank-quotes
  2  https://www.brainyquote.com/authors/arthur-schopenhauer-quotes
  2  https://www.brainyquote.com/authors/benjamin-franklin-quotes
  2  https://www.brainyquote.com/authors/bertrand-russell-quotes
  2  https://www.brainyquote.com/authors/bill-gates-quotes
  [...]
```

This command output all outging links found on a page #1 as well as theirs page type IDs. In our case we tell crab that page type for all those pages is 2. Which means that crab will be using different parser for parsing and navigating those pages.

Now let's write all this outgoing links to the database:

```
$ crab navigate-all
```

This command traverse all downloaded pages, apply navigation rules and writes discovered pages back to the database. After that we need to run crawler to downloaded them

```
$ crab run-crawler
```

Now let's check for a page with Alan Watts quotes:

```
$ crab list-pages | grep -i watts
      3        2      0  downloaded       https://www.brainyquote.com/authors/alan-watts-quotes

```

So it's page #3 with page type id 2. Let's create new parser for those type of pages. Let's create file `parser_author.py` with followinf content:

```python
from bs4 import BeautifulSoup
TYPE_ID: int = 2

def parse(content: str) -> dict[str, str]:
    html = BeautifulSoup(content, 'html.parser')
    result = {}
    id = 0
    title = html.select_one("nav h1")
    if title:
      result['title'] = title.text.strip()
    for quote in html.select('#quotesList a div'):
      id += 1
      result[f"quote-{id}"] = quote.text.strip()
    return result
```

All parser filenames must start with `parser_` prefix and contains `TYPE_ID` constant indicating for which pages this parser is suposed for.

Now let's run parser logic on a page

```
$ crab parse 3
quote-29: The reason we want to go on and on is because we live in an impoverished present.
quote-47: The moralist is the person who tells people that they ought to be unselfish, when they still feel like egos, and his efforts are always and invariably futile.
quote-11: Never pretend to a love which you do not actually feel, for love is not ours to command.
quote-32: The myths underlying our culture and underlying our common sense have not taught us to feel identical with the universe, but only parts of it, only in it, only confronting it - aliens.
quote-2: To have faith is to trust yourself to the water. When you swim you don't grab hold of the water, because if you do you will sink and drown. Instead you relax, and float.
quote-12: But the attitude of faith is to let go, and become open to truth, whatever it might turn out to be.
quote-20: But I'll tell you what hermits realize. If you go off into a far, far forest and get very quiet, you'll come to understand that you're connected with everything.
quote-45: If you study the writings of the mystics, you will always find things in them that appear to be paradoxes, as in Zen, particularly.
quote-43: Buddhism has in it no idea of there being a moral law laid down by somekind of cosmic lawgiver.
quote-14: So then, the relationship of self to other is the complete realization that loving yourself is impossible without loving everything defined as other than yourself.
quote-7: The ego is nothing other than the focus of conscious attention.
```

### Exporting as a CSV

```
$ crab export-csv
```

Exports all the quotes in a CSV format

### Running parser in a wild

So when you are write all the logic for navigating pages you need basically do following steps:

1. `crab navigate all` - will run naviagtion rules on all the pages and discover new links
2. `crab run-crawler --navigate` to downloaded all the pages. Note `--navigate` flag. By default crawler will download pages already in database but will not apply navigation rules to downloaded pages, so no new pages will be discovered. But if you pass this flag downloading and discovering will run simultaneiously.