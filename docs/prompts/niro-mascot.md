# Niro mascot generation prompt

This file defines the canonical visual identity for **Niro**, the official black-cat mascot of the Nilo programming language.

Use the repository artwork at `docs/assets/niro-mascot.svg` as the primary visual reference whenever the image model supports reference images.

## Canonical character specification

- Character: a small, friendly black cat named **Niro**
- Silhouette: chibi proportions, large head, compact round body, short legs, expressive raised paws
- Fur: deep black with subtle blue-black highlights
- Eyes: large emerald-green eyes with bright aqua reflections
- Forehead: one glowing geometric **N** mark, always the same shape and orientation
- Accents: emerald inner ears, paw pads, collar details, and tail tip
- Collar: slim dark-tech collar with a silver hexagonal tag containing an emerald `N`
- Personality: curious, cheerful, clever, helpful, slightly mischievous
- Theme: clean cyber technology, programming, creativity, discovery
- Brand colors: `#00E5C0`, `#00BFA6`, `#071317`, `#F7FBFB`

## Master prompt (English)

```text
Create a polished official mascot illustration for the Nilo programming language.

The character is Niro, a small adorable black cat with consistent chibi proportions: a large round head, compact body, short legs, oversized expressive paws, and a lively curved tail. Niro has glossy blue-black fur, very large emerald-green eyes with aqua highlights, emerald inner ears, emerald paw pads, and an emerald glowing tail tip. A clean geometric glowing N symbol is centered on the forehead and must remain identical in shape, scale, and orientation across every image. Niro wears a slim futuristic dark collar with a silver hexagonal pendant containing a small emerald N.

Expression and pose: friendly, curious, energetic, intelligent, and welcoming. The pose should communicate that Niro enjoys discovering ideas and finding bugs in code.

Visual style: premium modern anime-inspired mascot art, crisp linework, soft cel shading, subtle glossy highlights, balanced proportions, strong readable silhouette, professional open-source software branding, cyber-tech details used sparingly, emerald and black color palette, transparent background, no text, no watermark.

Keep the character model fully consistent with the official Niro reference artwork. Preserve the exact forehead N, eye color, paw-pad color, collar, pendant, tail-tip color, head-to-body ratio, ear shape, and facial proportions.
```

## Master prompt (Japanese)

```text
Niloプログラミング言語の公式マスコット「Niro（ニロ）」を、高品質なキャラクターイラストとして生成する。

ニロは、小さくて親しみやすい黒猫。大きな丸い頭、コンパクトな体、短い手足、大きく表情豊かな肉球、元気にカーブした尻尾を持つ、統一されたデフォルメ体型。毛色は青みを含む艶のある黒。瞳は大きなエメラルドグリーンで、アクア色の光が映り込んでいる。耳の内側、肉球、尻尾の先端はエメラルドグリーン。額の中央には、毎回まったく同じ形・大きさ・向きの、幾何学的に発光する「N」マークがある。首には未来的な細いダークカラーの首輪と、エメラルド色のNが入った銀色の六角形ペンダントを付ける。

性格と表情は、好奇心旺盛、明るい、賢い、親切、少しいたずら好き。アイデアを見つけたり、コードのバグを発見したりすることが好きだと伝わるポーズ。

画風は、現代的で上質なアニメ調マスコット。くっきりした線、柔らかなセル塗り、控えめな光沢、読み取りやすいシルエット、OSSプロジェクトに適したプロフェッショナルな仕上がり。サイバーテック要素は控えめに使い、エメラルド×ブラックで統一。背景は完全透明。文字、透かし、ロゴの追加は禁止。

公式のNiro参考画像に合わせ、額のN、瞳、肉球、首輪、ペンダント、尻尾の先、頭身、耳の形、顔の比率を必ず維持する。
```

## Negative prompt

```text
photorealistic cat, realistic animal anatomy, human body, humanoid hands, extra limbs, extra paws, missing tail, multiple tails, malformed paws, asymmetrical eyes, different eye colors, red eyes, blue eyes, white cat, gray cat, long realistic fur, oversized accessories, armor, weapon, horror, aggressive expression, dirty line art, muddy colors, low contrast, flat lifeless eyes, distorted face, duplicate forehead symbol, incorrect letter, text, caption, speech bubble, watermark, signature, logo, opaque background, busy background
```

## Composition presets

### README mascot

```text
Full-body Niro, front three-quarter view, both paws raised in a welcoming pose, cheerful open-mouth smile, transparent background, centered composition, generous clear space around the silhouette, suitable for display at 260 to 360 pixels wide in a GitHub README.
```

### Bug finder

```text
Niro inspecting a tiny stylized software bug through a magnifying glass, curious and excited expression, one paw pointing at the discovery, transparent background, no text.
```

### Thinking

```text
Niro sitting with one paw under the chin, thoughtful expression, a few small emerald code symbols floating nearby, transparent background, no text.
```

### Sticker

```text
Niro with an energetic full-body pose and exaggerated readable expression, thick white outer stroke, bold simple silhouette, transparent background, designed for a 370 x 320 pixel chat sticker, no text unless explicitly requested.
```

## Consistency checklist

Before accepting a generated image, verify all of the following:

1. The character is a black cat, not a human or generic animal.
2. The forehead contains exactly one correctly oriented geometric `N`.
3. Both eyes are emerald green with matching proportions.
4. The ear interiors, paw pads, and tail tip use the same emerald family.
5. The silver hexagonal pendant contains one emerald `N`.
6. The head-to-body ratio and ear shape match the reference.
7. No extra limbs, paws, tails, accessories, text, or watermark are present.
8. The background is transparent when the asset is intended for documentation or stickers.

## Recommended workflow

1. Start from `docs/assets/niro-mascot.svg` as the character reference.
2. Generate one neutral reference pose first.
3. Approve the face, forehead mark, collar, and color palette before generating variations.
4. Use the approved result as the reference for all later expressions and poses.
5. Export documentation assets as transparent PNG or SVG and keep editable source files outside release packages.
