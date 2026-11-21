<?php
$a = getopt('', ['ids::', 'colors::', 'out:', 'size::', 'view::', 'rig::', 'stance::', 'exact::', 'smooth::', 'scale::', 'pose::', 'item::', 'with::', 'zoom::', 'yaw::', 'pitch::']);
$vr = getenv('VANILLA_DIR') ?: '/var/www/html/core/vanilla-rcc';
require_once $vr . '/src/Vanilla.php';
@ini_set('memory_limit', '1500M');
$root = getenv('ASSET_DIR') ?: '/var/www/assets';
$tmp = sys_get_temp_dir() . '/swirl-php-cache-' . getmypid();
@mkdir($tmp, 0700, true);
$av = new \Vanilla\Avatar([
    'assetBase' => 'http://127.0.0.1/asset/?id=',
    'cacheDir' => $tmp,
    'exact' => !empty($a['exact']),
    'fetcher' => function ($id) use ($root) {
        $id = (string)$id;
        if (!ctype_digit($id)) return null;
        foreach (["$root/assets/", "$root/cache/"] as $d) {
            $f = $d . (int)$id;
            if (is_file($f)) {
                $b = @file_get_contents($f);
                if ($b !== false && $b !== '') return $b;
            }
        }
        return null;
    },
]);
$req = ['renderer' => '2012', 'size' => $a['size'] ?? '352x352', 'view' => $a['view'] ?? 'body'];
$ids = isset($a['ids']) && $a['ids'] !== '' ? explode(',', $a['ids']) : [];
$req['ids'] = $ids;
if (!empty($a['colors'])) $req['colors'] = array_map('intval', explode(',', $a['colors']));
$req['rig'] = $a['rig'] ?? 'R6';
if (!empty($a['item'])) { unset($req['ids']); $req['item'] = $a['item']; if (!empty($a['with'])) $req['with'] = explode(',', $a['with']); }
if (!empty($a['scale'])) { $sc = []; foreach (explode(',', $a['scale']) as $kv) { [$k, $v] = explode(':', $kv); $sc[$k] = (float)$v; } $req['scale'] = $sc; }
if (!empty($a['pose'])) $req['pose'] = $a['pose'];
if (isset($a['stance'])) $req['stance'] = $a['stance'];
if (isset($a['smooth']) && $a['smooth'] === '0') $req['smooth'] = false;
foreach (['zoom', 'yaw', 'pitch'] as $k) if (isset($a[$k])) $req[$k] = $a[$k];
$r = $av->render($req);
file_put_contents($a['out'], $r['png']);
foreach ($r['notes'] as $n) fwrite(STDERR, "note: $n\n");
foreach (glob($tmp . '/*') ?: [] as $f) @unlink($f);
@rmdir($tmp);
