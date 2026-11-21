<?php
require_once '/var/www/swirl/tools/parity/php-ref/src/Vanilla.php';
$t = \Vanilla\Rbxm::parse(file_get_contents($argv[1]));
function w($x, $d) {
    echo str_repeat(' ', $d) . $x->cls . ' ' . $x->name . ' ' . count($x->props) . "\n";
    foreach ($x->children as $c) w($c, $d + 1);
}
foreach ($t['roots'] as $r) w($r, 0);
