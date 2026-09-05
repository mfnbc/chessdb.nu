<map version="1.0.1">
<node TEXT="chessdb shakmaty architecture (2026-09-03)">

  <node TEXT="Layer 1 -- chessdb leaf commands (rust, ~1:1 shakmaty)" POSITION="right">
    <node TEXT="geom-attacks">
      <node TEXT="shakmaty::attacks::attacks(sq, piece, occupied)"/>
      <node TEXT="one dispatcher for every role, occupied always explicit"/>
    </node>
    <node TEXT="geom-ray">
      <node TEXT="shakmaty::attacks::ray(a, b)"/>
    </node>
    <node TEXT="geom-between">
      <node TEXT="shakmaty::attacks::between(a, b)"/>
    </node>
    <node TEXT="geom-aligned">
      <node TEXT="shakmaty::attacks::aligned(a, b, c)"/>
    </node>
    <node TEXT="board-pieces">
      <node TEXT="Board::occupied()"/>
      <node TEXT="Board::by_color(color)"/>
      <node TEXT="Board::by_role(role)"/>
      <node TEXT="Board::by_piece(piece)"/>
    </node>
    <node TEXT="board-piece-at">
      <node TEXT="Board::piece_at(sq)"/>
    </node>
    <node TEXT="square-is-light">
      <node TEXT="Square::is_light()"/>
      <node TEXT="no FEN needed at all -- pure square geometry"/>
    </node>
    <node TEXT="legal-moves">
      <node TEXT="Position::legal_moves()"/>
      <node TEXT="SanPlus::from_move() -- real +/# suffixes, fixed 2026-09-03"/>
    </node>
    <node TEXT="checker-summary">
      <node TEXT="Position::checkers()"/>
      <node TEXT="Position::is_check() / is_checkmate()"/>
    </node>
    <node TEXT="fen-info">
      <node TEXT="Position::halfmoves() / fullmoves()"/>
      <node TEXT="Position::ep_square()"/>
      <node TEXT="Position::castles()"/>
      <node TEXT="Position::is_stalemate() / is_insufficient_material()"/>
    </node>
    <node TEXT="apply-uci">
      <node TEXT="Position::play()"/>
    </node>
    <node TEXT="hugm-eval">
      <node TEXT="tactical/positional detectors (sensor_report)"/>
      <node TEXT="NOT a shakmaty leaf -- this crate's own domain analysis, feeds full-report directly"/>
    </node>
  </node>

  <node TEXT="Layer 2 -- nu composition (shakmaty_compose.nu)" POSITION="right">
    <node TEXT="attacks-to">
      <node TEXT="composes: geom-attacks + board-pieces + board-piece-at"/>
      <node TEXT="replaces removed rust command chessdb square-attackers"/>
    </node>
    <node TEXT="attacks-from">
      <node TEXT="composes: geom-attacks + board-piece-at"/>
      <node TEXT="replaces removed rust command chessdb square-control"/>
    </node>
    <node TEXT="swap-list">
      <node TEXT="composes: attacks-to + board-pieces, recursive"/>
      <node TEXT="x-ray removal is just a nu `where` filter over occupancy, no rust loop"/>
      <node TEXT="replaces removed rust command chessdb square-swap-list"/>
    </node>
    <node TEXT="board-probe">
      <node TEXT="composes: geom-attacks + board-pieces + board-piece-at + square-is-light + fen-info + checker-summary + legal-moves"/>
      <node TEXT="O(pieces) round trips, inverted in nu -- not O(64 x pieces)"/>
      <node TEXT="replaces removed rust command chessdb board-probe"/>
    </node>
    <node TEXT="full-report">
      <node TEXT="composes: board-probe + hugm-eval sensor_report"/>
      <node TEXT="filtered through strip-scores"/>
    </node>
    <node TEXT="strip-scores">
      <node TEXT="generic recursive filter: drops any key matching score|_cp$|centipawn|consequence"/>
      <node TEXT="blunt name-pattern filter, not a hand-audited allowlist (explicit priority: lowest of three)"/>
    </node>
  </node>

  <node TEXT="Layer 3 -- scripts/play/*.nu (the tools)" POSITION="left">
    <node TEXT="control_map.nu">
      <node TEXT="built on attacks-from"/>
    </node>
    <node TEXT="attackers_map.nu">
      <node TEXT="built on attacks-to"/>
    </node>
    <node TEXT="square_swap_list.nu">
      <node TEXT="built on swap-list"/>
    </node>
    <node TEXT="board_probe.nu">
      <node TEXT="built on board-probe"/>
    </node>
    <node TEXT="full_report.nu">
      <node TEXT="built on full-report"/>
      <node TEXT="THE comprehensive report -- position-eval SKILL.md reads from this"/>
    </node>
    <node TEXT="check_move.nu / check_move_2ply.nu / calc_line.nu">
      <node TEXT="built on apply-uci + hugm-eval directly (not through the compose layer)"/>
    </node>
    <node TEXT="forcing_moves.nu">
      <node TEXT="built on legal-moves"/>
    </node>
    <node TEXT="control_overlap.nu">
      <node TEXT="built on attack-summary (whole-board, kept as-is, not decomposed)"/>
    </node>
    <node TEXT="material.nu">
      <node TEXT="built on hugm-eval material.balance (raw counts only)"/>
    </node>
  </node>

</node>
</map>
