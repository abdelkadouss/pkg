export def main [input: string] {
  if ($env.dir? | is-not-empty) {
    mkdir out;
    $"this thing should be: ($input)" o> out/entry_point
    return "./out,x.x.x,./out/entry_point"


  } else {
    $"this thing should be: ($input)" o> out

    return "./out,x.x.x"

  }



}
