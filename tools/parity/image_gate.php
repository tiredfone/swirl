<?php
$d = $argv[1];
$im = imagecreatetruecolor(64, 48);
imagealphablending($im, false);
imagesavealpha($im, true);
for ($y = 0; $y < 48; $y++) {
    for ($x = 0; $x < 64; $x++) {
        imagesetpixel($im, $x, $y, imagecolorallocatealpha($im, $x * 4, $y * 5, ($x * $y) % 256, ($x > 50) ? 127 : (($y > 40) ? 60 : 0)));
    }
}
imagewebp($im, "$d/t.webp", 90);
imagegif($im, "$d/t.gif");
imagebmp($im, "$d/t.bmp");
foreach (['webp', 'gif', 'bmp'] as $t) {
    $g = imagecreatefromstring(file_get_contents("$d/t.$t"));
    imagepalettetotruecolor($g);
    imagealphablending($g, false);
    imagesavealpha($g, true);
    imagepng($g, "$d/g_$t.png");
}
