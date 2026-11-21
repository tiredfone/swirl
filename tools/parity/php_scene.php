<?php
$a = getopt('', ['kind:', 'file:', 'size:', 'outsize:', 'era::', 'out:', 'content-dir::']);
require_once '/var/www/html/core/vanilla-rcc/src/Vanilla.php';
@ini_set('memory_limit', '1500M');
$root = '/var/www/assets';
$tmp = sys_get_temp_dir() . '/swirl-php-scene-' . getmypid();
@mkdir($tmp, 0700, true);
$av = new \Vanilla\Avatar([
    'assetBase' => 'http://127.0.0.1/asset/?id=', 'cacheDir' => $tmp,
    'fetcher' => function ($id) use ($root, $a) {
        $id = (string)$id;
        if (strncmp($id, 'solarterrain/', 13) === 0 && !empty($a['content-dir'])) { $f = $a['content-dir'] . '/' . $id; return is_file($f) ? file_get_contents($f) : null; }
        if (!ctype_digit($id)) return null;
        foreach (["$root/assets/", "$root/cache/"] as $d) {
            $f = $d . (int)$id;
            if (is_file($f)) { $b = @file_get_contents($f); if ($b !== false && $b !== '') return $b; }
        }
        return null;
    },
]);
[$w, $h] = array_map('intval', explode('x', $a['size']));
[$ow, $oh] = array_map('intval', explode('x', $a['outsize'] ?? $a['size']));
$r = $av->renderScene(['kind' => $a['kind'], 'bytes' => file_get_contents($a['file']), 'width' => $w, 'height' => $h, 'outWidth' => $ow, 'outHeight' => $oh, 'era' => (int)($a['era'] ?? 2012)]);
file_put_contents($a['out'], $r['png']);
foreach ($r['notes'] as $n) fwrite(STDERR, "note: $n\n");
foreach (glob($tmp . '/*') ?: [] as $f) @unlink($f);
@rmdir($tmp);
