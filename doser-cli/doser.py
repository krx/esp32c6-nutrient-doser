import json
import click
import re
from urllib.parse import urlparse
from pathlib import Path
from requests import post

FEED_CHART = Path(__file__).parent / "floragro_chart.json"
CHARTS: dict = json.loads(FEED_CHART.read_text())


class URL(click.ParamType):
    name = "url"

    def convert(self, value, param, ctx):
        if not isinstance(value, tuple):
            parsed = urlparse(value)
            if parsed.scheme not in ("http", "https"):
                self.fail(
                    f"invalid URL scheme ({parsed.scheme}). Only HTTP URLs are allowed",
                    param,
                    ctx,
                )
        return value


@click.command()
@click.option(
    "--host",
    type=URL(),
    default="http://nutrient-doser.lan",
    help="host[:port] of doser to connect to",
)
@click.option(
    "--chart",
    help="name of chart to use",
    required=True,
    type=click.Choice(CHARTS.keys()),
)
@click.option("--stage", help="growth stage to reference in chart", required=True)
@click.option(
    "--amount",
    help="Target amount of solution to mix (allowed units: ml, L, gal)",
    required=True,
)
def main(host, chart, stage, amount):
    ch = CHARTS[chart]
    if stage not in ch:
        print(f"Invalid growth stage: {stage}. Available stages are {list(ch.keys())}")
        return -1

    if (match := re.match(r"(\d+(?:\.\d+)?) *(ml|l|gal)", amount, re.I)) is None:
        print("Invalid amount entered: {amount}")
        return -1

    target_val = float(match.group(1))
    target_unit = match.group(2).capitalize()

    nutrients = []
    for i, (name, mlpg) in enumerate(ch[stage].items()):
        nutrients.append({"motor_idx": i, "name": name, "ml_per_gal": mlpg})

    payload = {
        "nutrients": nutrients,
        "target_amount": target_val,
        "target_unit": target_unit,
    }

    post(f"{host}/dose", json=payload)


if __name__ == "__main__":
    main()
