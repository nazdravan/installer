#!/usr/bin/env python3
# Construiește paginile în română, engleză și rusă din src/template.html și src/i18n/*.json.
# Româna e la rădăcină (site/index.html), celelalte în site/en/ și site/ru/. Doar biblioteca standard.
import json
import re
from pathlib import Path

HERE = Path(__file__).parent
LANGS = ["ro", "en", "ru"]
PLACEHOLDER = re.compile(r"\{\{(\w+)\}\}")


def render(template, values):
    """Înlocuiește {{cheie}}; o cheie lipsă oprește build-ul, ca să nu apară acolade în pagină."""
    def value(m):
        key = m.group(1)
        if key not in values:
            raise SystemExit(f"lipsește traducerea „{key}” ({values.get('lang')})")
        return values[key]
    return PLACEHOLDER.sub(value, template)


def main():
    template = (HERE / "src/template.html").read_text(encoding="utf-8")
    keys = set(PLACEHOLDER.findall(template))
    for lang in LANGS:
        values = json.loads((HERE / f"src/i18n/{lang}.json").read_text(encoding="utf-8"))
        root = "" if lang == "ro" else "../"
        values |= {
            "root": root,
            "home": root or "./",
            **{f"current_{l}": 'aria-current="page"' if l == lang else "" for l in LANGS},
        }
        unused = set(values) - keys - {"lang"}
        if unused:
            print(f"{lang}: chei nefolosite: {', '.join(sorted(unused))}")
        out = HERE / ("index.html" if lang == "ro" else f"{lang}/index.html")
        out.parent.mkdir(exist_ok=True)
        out.write_text(render(template, values), encoding="utf-8")
        print(f"{lang}: {out.relative_to(HERE)}")


if __name__ == "__main__":
    main()
