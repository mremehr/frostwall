#!/usr/bin/env python3
"""Generate CLIP text embeddings for wallpaper categories.

Outputs both Rust source (for reference) and compact binary format.
Run with: uv run --with torch --with transformers scripts/gen_embeddings.py
"""
import os
import struct
import sys
from pathlib import Path

# Disable progress bars
os.environ["HF_HUB_DISABLE_PROGRESS_BARS"] = "1"
os.environ["TRANSFORMERS_NO_ADVISORY_WARNINGS"] = "1"

import warnings

warnings.filterwarnings("ignore")

import torch
from transformers import CLIPModel, CLIPProcessor, logging

logging.set_verbosity_error()

# Each category has 2+ text prompts that are averaged into one embedding.
# More prompts = more robust embedding.  Prompts should describe what the
# image *looks like*, not metadata.
CATEGORIES = {
    # ── Nature & Scenery ──
    "nature": [
        "a photograph of natural scenery",
        "beautiful nature landscape with plants",
    ],
    "forest": [
        "a dense forest with tall trees",
        "woodland photography with green foliage",
    ],
    "ocean": ["ocean waves and sea water", "beach and coastline photography"],
    "mountain": [
        "mountain peaks and alpine landscape",
        "rocky mountains with snow",
    ],
    "desert": [
        "sandy desert landscape with dunes",
        "arid desert environment",
    ],
    "tropical": [
        "tropical beach with palm trees",
        "exotic tropical paradise",
    ],
    "snow": [
        "snowy winter landscape with ice",
        "cold frozen winter scenery covered in white snow",
    ],
    "rain": [
        "rainy weather with wet streets and puddles",
        "atmospheric rain falling on a moody scene",
    ],
    "autumn": [
        "autumn foliage with red orange and yellow leaves",
        "fall season landscape with warm tones",
    ],
    "flowers": [
        "close up of beautiful flowers blooming",
        "colorful flower field or garden photography",
    ],
    "underwater": [
        "underwater ocean scene with fish and coral",
        "deep sea aquatic photography",
    ],
    "sky": [
        "dramatic cloud formations in the sky",
        "wide open sky with atmospheric clouds",
    ],
    "waterfall": [
        "waterfall cascading over rocks",
        "scenic waterfall in lush nature setting",
    ],
    # ── Urban & Architecture ──
    "city": [
        "urban cityscape with buildings and skyscrapers",
        "city skyline at night",
    ],
    "urban": ["urban street scene", "metropolitan urban environment"],
    "architecture": [
        "architectural photography of buildings",
        "beautiful architecture",
    ],
    "ruins": [
        "ancient ruins and overgrown abandoned architecture",
        "crumbling medieval ruins in a fantasy setting",
    ],
    "castle": [
        "medieval castle or fortress on a hill",
        "grand fantasy castle with towers and turrets",
    ],
    "neon": [
        "neon lights glowing in a dark city",
        "colorful neon signs and reflections at night",
    ],
    # ── Abstract & Minimal ──
    "abstract": [
        "abstract art with geometric patterns",
        "surreal abstract digital artwork",
    ],
    "minimal": [
        "minimalist design with clean simple lines",
        "sparse minimalist composition",
    ],
    "geometric": [
        "geometric patterns and shapes",
        "mathematical geometric artwork",
    ],
    # ── Style / Era ──
    "vintage": [
        "vintage retro style photography",
        "old fashioned vintage aesthetic",
    ],
    "retro": [
        "retro 80s 90s aesthetic",
        "synthwave retro style artwork",
    ],
    "steampunk": [
        "steampunk machinery with brass gears and steam",
        "victorian era steampunk aesthetic with mechanical elements",
    ],
    "gothic": [
        "dark gothic architecture and atmosphere",
        "spooky gothic style with gargoyles and cathedrals",
    ],
    "art_nouveau": [
        "art nouveau decorative illustration style",
        "ornate flowing organic art nouveau design",
    ],
    "vaporwave": [
        "vaporwave aesthetic with pink purple and blue",
        "retro vaporwave style with greek statues and palm trees",
    ],
    "watercolor": [
        "soft watercolor painting with flowing colors",
        "delicate watercolor art illustration",
    ],
    "oil_painting": [
        "classical oil painting with rich brush strokes",
        "traditional oil painting fine art style",
    ],
    "line_art": [
        "clean line art illustration with ink outlines",
        "black and white line drawing artwork",
    ],
    "3d_render": [
        "photorealistic 3D rendered scene",
        "computer generated 3D artwork with realistic lighting",
    ],
    "photography": [
        "professional photography with sharp detail",
        "real world photograph with natural lighting",
    ],
    "illustration": [
        "hand drawn digital illustration artwork",
        "artistic illustration with bold lines and colors",
    ],
    "digital_art": [
        "polished digital art created on a computer",
        "modern digital artwork with vivid colors and detail",
    ],
    # ── Mood / Atmosphere ──
    "dark": [
        "dark moody atmosphere",
        "shadowy low-key scene at night",
    ],
    "bright": [
        "bright and vibrant colorful scene",
        "sunny cheerful high-key photography",
    ],
    "sunset": [
        "sunset sky with golden orange colors",
        "golden hour twilight photography",
    ],
    "pastel": ["soft pastel colors", "gentle muted pastel tones"],
    "vibrant": [
        "vibrant saturated colors",
        "colorful high contrast imagery",
    ],
    "cozy": [
        "cozy warm comfortable interior",
        "homey relaxing atmosphere",
    ],
    "serene": [
        "peaceful serene calm tranquil scene",
        "relaxing meditative quiet landscape",
    ],
    "dramatic": [
        "dramatic intense scene with strong contrast",
        "epic dramatic lighting with dark clouds and light rays",
    ],
    "horror": [
        "creepy horror scene with eerie atmosphere",
        "dark disturbing unsettling nightmare imagery",
    ],
    # ── Anime & Manga ──
    "anime": [
        "anime art style illustration",
        "japanese animation manga artwork",
    ],
    "chibi": [
        "cute chibi character with big head small body",
        "kawaii super deformed chibi anime style",
    ],
    "mecha": [
        "giant robot mecha anime illustration",
        "mechanical mech suit from japanese anime",
    ],
    "shoujo": [
        "soft sparkly shoujo manga art with flowers",
        "romantic shoujo anime style with elegant characters",
    ],
    # ── Fantasy & Sci-fi ──
    "fantasy": [
        "fantasy magical landscape",
        "mythical enchanted fantasy artwork",
    ],
    "sci_fi": [
        "futuristic science fiction scene with advanced technology",
        "sci-fi spaceship or space station environment",
    ],
    "cyberpunk": [
        "cyberpunk neon city aesthetic",
        "futuristic technology neon lights",
    ],
    "dragon": [
        "dragon mythical creature flying or breathing fire",
        "epic fantasy dragon illustration",
    ],
    "samurai": [
        "japanese samurai warrior with katana sword",
        "samurai in traditional armor artwork",
    ],
    "magic": [
        "magical spell casting with glowing runes and energy",
        "wizard or witch using mystical magical powers",
    ],
    "space": [
        "outer space with stars and galaxies",
        "cosmic nebula and planets",
    ],
    # ── Orientation / Composition ──
    "portrait": [
        "portrait orientation vertical image",
        "tall vertical composition",
    ],
    "landscape_orientation": [
        "landscape orientation horizontal wide image",
        "panoramic wide view",
    ],
}


def main():
    project_dir = Path(__file__).parent.parent
    output_bin = project_dir / "data" / "embeddings.bin"

    print("Loading CLIP model...", file=sys.stderr)
    model = CLIPModel.from_pretrained("openai/clip-vit-base-patch32")
    processor = CLIPProcessor.from_pretrained("openai/clip-vit-base-patch32")

    print(f"Generating embeddings for {len(CATEGORIES)} categories...", file=sys.stderr)
    embeddings = {}
    for cat, descs in CATEGORIES.items():
        inputs = processor(text=descs, return_tensors="pt", padding=True, truncation=True)
        with torch.no_grad():
            outputs = model.text_model(
                input_ids=inputs["input_ids"],
                attention_mask=inputs["attention_mask"],
            )
            pooled = outputs.pooler_output
            text_embeds = model.text_projection(pooled)
            avg = text_embeds.mean(dim=0)
            avg = avg / avg.norm()
            embeddings[cat] = avg.tolist()

    # Write binary format
    output_bin.parent.mkdir(parents=True, exist_ok=True)
    with open(output_bin, "wb") as f:
        f.write(struct.pack("<I", len(embeddings)))
        for cat in sorted(embeddings.keys()):
            emb = embeddings[cat]
            name_bytes = cat.encode("utf-8")
            f.write(struct.pack("<I", len(name_bytes)))
            f.write(name_bytes)
            for v in emb:
                f.write(struct.pack("<f", v))

    size_kb = output_bin.stat().st_size / 1024
    print(f"Wrote {len(embeddings)} categories to {output_bin} ({size_kb:.1f} KB)", file=sys.stderr)

    # Print summary
    for cat in sorted(embeddings.keys()):
        print(f"  {cat}")


if __name__ == "__main__":
    main()
