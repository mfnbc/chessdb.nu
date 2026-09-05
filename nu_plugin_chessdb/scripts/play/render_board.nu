#!/usr/bin/env nu
# Usage: nu render_board.nu '<fen>' <output.svg>
#
# Renders a FEN as an SVG chessboard using Unicode piece glyphs, built
# directly from board_overlay.nu's fen-to-board (the same square->{color,
# role} record every other tool in this directory already trusts) --
# nothing hand-parsed from the FEN string itself. This is NOT a chessdb
# product feature (chessdb itself never renders -- see
# chessdb_board_probe) -- it's a private analysis aid: a way to actually
# look at a position with real vision instead of only ever reading it as
# structured text/JSON, tried 2026-09-04 per explicit user direction to
# apply that part of the model's cognition rather than only discuss it.
#
# Output is SVG; convert to PNG separately with `rsvg-convert` (no
# python-chess/cairosvg available in this environment) before reading it
# as an image.
use ./board_overlay.nu *

def glyph [color: string, role: string] {
    match [$color $role] {
        [white Pawn] => "♙", [white Knight] => "♘", [white Bishop] => "♗",
        [white Rook] => "♖", [white Queen] => "♕", [white King] => "♔",
        [black Pawn] => "♟", [black Knight] => "♞", [black Bishop] => "♝",
        [black Rook] => "♜", [black Queen] => "♛", [black King] => "♚",
        _ => "?"
    }
}

def main [fen: string, out: string] {
    let board = (fen-to-board $fen)
    let files = [a b c d e f g h]
    let sq = 90
    let margin = 40
    let size = ($sq * 8 + $margin * 2)

    mut squares_svg = []
    mut pieces_svg = []
    mut labels_svg = []

    for rank in 1..8 {
        let display_rank = (9 - $rank)  # rank 8 at top
        for fi in 0..7 {
            let file = ($files | get $fi)
            let square_name = $"($file)($display_rank)"
            let x = ($margin + $fi * $sq)
            let y = ($margin + ($rank - 1) * $sq)
            # Verified against two anchors: a1 must be dark (h1 light, since
            # they're 7 files apart = odd = opposite colors), and "queen on
            # her own color" (white queen d1 on light, black queen d8 on
            # dark) -- both agreed the original `== 1` parity was inverted.
            let is_light = (($fi + $display_rank) mod 2 == 0)
            let fill = if $is_light { "#EEEED2" } else { "#769656" }
            $squares_svg = ($squares_svg | append $"<rect x=\"($x)\" y=\"($y)\" width=\"($sq)\" height=\"($sq)\" fill=\"($fill)\"/>")

            let occ = ($board | get -o $square_name)
            if $occ != null {
                let g = (glyph $occ.color $occ.role)
                let cx = ($x + $sq / 2)
                let cy = ($y + $sq / 2 + 30)
                let piece_color = if $occ.color == "white" { "#FFFFFF" } else { "#1a1a1a" }
                let stroke = if $occ.color == "white" { "#1a1a1a" } else { "none" }
                $pieces_svg = ($pieces_svg | append $"<text x=\"($cx)\" y=\"($cy)\" font-size=\"64\" text-anchor=\"middle\" fill=\"($piece_color)\" stroke=\"($stroke)\" stroke-width=\"1.5\">($g)</text>")
            }
        }
        let y = ($margin + ($rank - 1) * $sq + $sq / 2 + 10)
        $labels_svg = ($labels_svg | append $"<text x=\"18\" y=\"($y)\" font-size=\"24\" fill=\"#333\">($display_rank)</text>")
    }
    for fi in 0..7 {
        let file = ($files | get $fi)
        let x = ($margin + $fi * $sq + $sq / 2)
        $labels_svg = ($labels_svg | append $"<text x=\"($x)\" y=\"($size - 12)\" font-size=\"24\" fill=\"#333\" text-anchor=\"middle\">($file)</text>")
    }

    let svg = $"<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"($size)\" height=\"($size)\">
<rect width=\"($size)\" height=\"($size)\" fill=\"#FFFFFF\"/>
(($squares_svg | str join (char newline)))
(($pieces_svg | str join (char newline)))
(($labels_svg | str join (char newline)))
</svg>"

    $svg | save -f $out
    print $"wrote ($out)"
}
