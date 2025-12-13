"""
Generate placeholder sprites for the factory game.
All sprites are 32x32 pixels.
"""

from PIL import Image, ImageDraw


def create_player():
    """Create a player character - simple humanoid shape"""
    img = Image.new("RGBA", (32, 32), (0, 0, 0, 0))
    draw = ImageDraw.Draw(img)

    # Head
    draw.ellipse([12, 6, 20, 14], fill=(255, 220, 180, 255))

    # Body - blue shirt
    draw.rectangle([11, 14, 21, 24], fill=(50, 100, 200, 255))

    # Arms
    draw.rectangle([7, 15, 11, 22], fill=(50, 100, 200, 255))  # Left arm
    draw.rectangle([21, 15, 25, 22], fill=(50, 100, 200, 255))  # Right arm

    # Hands
    draw.ellipse([6, 20, 10, 24], fill=(255, 220, 180, 255))
    draw.ellipse([22, 20, 26, 24], fill=(255, 220, 180, 255))

    # Legs - dark pants
    draw.rectangle([12, 24, 15, 30], fill=(40, 40, 80, 255))  # Left leg
    draw.rectangle([17, 24, 20, 30], fill=(40, 40, 80, 255))  # Right leg

    # Eyes
    draw.rectangle([14, 9, 15, 10], fill=(0, 0, 0, 255))
    draw.rectangle([17, 9, 18, 10], fill=(0, 0, 0, 255))

    # Outline for visibility
    draw.ellipse([12, 6, 20, 14], outline=(200, 180, 150, 255), width=1)

    return img


def create_miner():
    """Create a mining machine - industrial gray box with drill"""
    img = Image.new("RGBA", (32, 32), (0, 0, 0, 0))
    draw = ImageDraw.Draw(img)

    # Main body - gray
    draw.rectangle([4, 4, 28, 28], fill=(100, 100, 100, 255))

    # Top section - darker
    draw.rectangle([6, 6, 26, 12], fill=(70, 70, 70, 255))

    # Drill/mining arm in center
    draw.rectangle([14, 14, 18, 26], fill=(60, 60, 60, 255))

    # Drill tip - yellow/gold
    draw.polygon([(14, 26), (18, 26), (16, 30)], fill=(200, 180, 50, 255))

    # Side panels for depth
    draw.rectangle([6, 14, 10, 26], fill=(80, 80, 80, 255))
    draw.rectangle([22, 14, 26, 26], fill=(80, 80, 80, 255))

    # Warning stripes on sides
    draw.rectangle([7, 15, 9, 17], fill=(220, 200, 50, 255))
    draw.rectangle([7, 19, 9, 21], fill=(220, 200, 50, 255))
    draw.rectangle([23, 15, 25, 17], fill=(220, 200, 50, 255))
    draw.rectangle([23, 19, 25, 21], fill=(220, 200, 50, 255))

    # Status lights
    draw.ellipse([8, 8, 11, 11], fill=(100, 200, 100, 255))  # Green light
    draw.ellipse([21, 8, 24, 11], fill=(200, 100, 100, 255))  # Red light

    # Border
    draw.rectangle([4, 4, 28, 28], outline=(50, 50, 50, 255), width=1)

    return img


def create_tile():
    """Create a ground tile - brown/gray with some texture"""
    img = Image.new("RGBA", (32, 32), (0, 0, 0, 0))
    draw = ImageDraw.Draw(img)

    # Base brown color
    draw.rectangle([0, 0, 31, 31], fill=(101, 85, 70, 255))

    # Add some texture with lighter/darker spots
    for x in range(0, 32, 8):
        for y in range(0, 32, 8):
            if (x + y) % 16 == 0:
                draw.rectangle([x, y, x + 7, y + 7], fill=(115, 95, 80, 255))

    # Border for visibility
    draw.rectangle([0, 0, 31, 31], outline=(80, 70, 60, 255), width=1)

    return img


def create_ore():
    """Create an ore deposit - dark metallic gray with crystals"""
    img = Image.new("RGBA", (32, 32), (0, 0, 0, 0))
    draw = ImageDraw.Draw(img)

    # Dark gray base
    draw.ellipse([4, 4, 27, 27], fill=(60, 60, 70, 255))

    # Add some ore "crystals" - lighter spots
    ore_spots = [(10, 8), (18, 12), (14, 18), (20, 20), (10, 22)]
    for x, y in ore_spots:
        draw.ellipse([x, y, x + 4, y + 4], fill=(120, 120, 140, 255))
        # Highlight
        draw.ellipse([x + 1, y + 1, x + 2, y + 2], fill=(160, 160, 180, 255))

    # Border
    draw.ellipse([4, 4, 27, 27], outline=(40, 40, 50, 255), width=2)

    return img


def create_crafter():
    """Create a crafting machine - blue/gray industrial box"""
    img = Image.new("RGBA", (32, 32), (0, 0, 0, 0))
    draw = ImageDraw.Draw(img)

    # Main body - blue-gray
    draw.rectangle([2, 2, 29, 29], fill=(80, 100, 130, 255))

    # Top panel - lighter
    draw.rectangle([4, 4, 27, 10], fill=(100, 120, 150, 255))

    # Side panels for depth
    draw.rectangle([4, 12, 8, 27], fill=(60, 80, 110, 255))
    draw.rectangle([23, 12, 27, 27], fill=(60, 80, 110, 255))

    # Center "processing" area
    draw.rectangle([10, 14, 21, 25], fill=(40, 60, 90, 255))

    # Add some "lights" or indicators
    draw.rectangle([6, 6, 8, 8], fill=(100, 200, 100, 255))
    draw.rectangle([23, 6, 25, 8], fill=(200, 100, 100, 255))

    # Border
    draw.rectangle([2, 2, 29, 29], outline=(40, 50, 70, 255), width=1)

    return img


def create_belt():
    """Create a belt belt - gray with yellow/black stripes"""
    img = Image.new("RGBA", (32, 32), (0, 0, 0, 0))
    draw = ImageDraw.Draw(img)

    # Base gray
    draw.rectangle([0, 8, 31, 23], fill=(80, 80, 80, 255))

    # Side rails - darker
    draw.rectangle([0, 8, 31, 10], fill=(50, 50, 50, 255))
    draw.rectangle([0, 21, 31, 23], fill=(50, 50, 50, 255))

    # Yellow/black stripes showing direction (moving right)
    for x in range(0, 32, 6):
        color = (220, 200, 50, 255) if (x // 6) % 2 == 0 else (40, 40, 40, 255)
        draw.rectangle([x, 11, min(x + 5, 31), 20], fill=color)

    # Arrow showing direction
    draw.polygon([(20, 14), (26, 16), (20, 18)], fill=(255, 255, 255, 200))

    return img


def create_curved_belt():
    """Create a curved belt - 90 degree turn from bottom to right"""
    img = Image.new("RGBA", (32, 32), (0, 0, 0, 0))
    draw = ImageDraw.Draw(img)

    # Create the curved belt path - quarter circle arc
    # The belt enters from bottom (270°) and exits to the right (0°)
    # Center point for the arc is at bottom-right corner
    # This means we draw from 180° to 270°

    # Draw the base belt surface (outer arc)
    # Bounding box positioned so the arc goes from bottom to right
    draw.pieslice([8, 8, 56, 56], 180, 270, fill=(80, 80, 80, 255))

    # Draw the inner rails (darker)
    # Outer rail
    draw.arc([8, 8, 56, 56], 180, 270, fill=(50, 50, 50, 255), width=3)
    # Inner rail
    draw.arc([21, 21, 43, 43], 180, 270, fill=(50, 50, 50, 255), width=3)

    # Create the curved stripe pattern
    # Draw arcs at different radii for the yellow/black pattern
    stripe_count = 6
    for i in range(stripe_count):
        angle_start = 180 + (i * 15)
        angle_end = angle_start + 12
        color = (220, 200, 50, 255) if i % 2 == 0 else (40, 40, 40, 255)

        # Draw stripe as a filled arc segment
        temp = Image.new("RGBA", (32, 32), (0, 0, 0, 0))
        temp_draw = ImageDraw.Draw(temp)
        temp_draw.pieslice([8, 8, 56, 56], angle_start, angle_end, fill=color)
        # Punch out the inner part
        temp_draw.pieslice([21, 21, 43, 43], angle_start, angle_end, fill=(0, 0, 0, 0))
        img.paste(temp, (0, 0), temp)

    # Add a curved arrow showing direction (bottom to right)
    # Arrow at about the middle of the curve (around 225°)
    draw.polygon([(18, 22), (14, 20), (18, 18)], fill=(255, 255, 255, 200))

    return img


def main():
    """Generate all sprite files"""
    sprites = {
        "player.png": create_player(),
        "miner.png": create_miner(),
        "tile.png": create_tile(),
        "ore.png": create_ore(),
        "crafter.png": create_crafter(),
        "belt.png": create_belt(),
        "belt_curved.png": create_curved_belt(),
    }

    output_dir = "assets/sprites"

    for filename, sprite in sprites.items():
        filepath = f"{output_dir}/{filename}"
        sprite.save(filepath)
        print(f"Created {filepath}")

    print("\nAll placeholder sprites generated successfully!")
    print("Each sprite is 32x32 pixels in PNG format.")


if __name__ == "__main__":
    main()
