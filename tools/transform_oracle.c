// Bit-exactness oracle for girc-core's VVC integer transforms.
//
// This program links Fraunhofer HHI's VVenC transform kernels directly and
// prints the forward and inverse transform of a fixed set of test blocks.
// `girc-core`'s test suite compares its output against this file, so any
// deviation from the reference implementation is caught by `cargo test`.
//
// Build and run (from the repository root, with VVenC's source available at
// $VVENC_SRC):
//
//   cc -O2 -I"$VVENC_SRC/source/Lib/CommonLib" \
//      tools/transform_oracle.c \
//      "$VVENC_SRC/source/Lib/CommonLib/TrQuant_EMT.cpp" \
//      -o build/transform_oracle
//   ./build/transform_oracle > tools/transform_oracle_vectors.txt
//
// The output is a Rust-source-compatible table so the test can include it
// directly.

#include <stdio.h>
#include <stdlib.h>
#include <string.h>

#include "Rom.h"
#include "TrQuant_EMT.h"

using namespace vvenc;

typedef int coeff_t;

#define MAX_N 32

// The VVC shift schedule, from TrQuant.cpp.
static const int TRANSFORM_MATRIX_SHIFT = 6;
static const int maxLog2TrDynamicRange = 15;
static const int bitDepth = 8;

static int log2i(int x)
{
  int n = 0;
  while ( ( 1 << n ) < x )
  {
    n++;
  }
  return n;
}

// Run the forward transform of the given type over an n x n block, using the
// same two-pass structure and shifts as TrQuant::xT.
static void forward(int type, int n, const coeff_t *src, coeff_t *dst)
{
  coeff_t block[MAX_N * MAX_N];
  coeff_t tmp[MAX_N * MAX_N];
  memcpy(block, src, sizeof(coeff_t) * n * n);

  const int shift_1st = ( log2i( n ) + bitDepth + TRANSFORM_MATRIX_SHIFT ) - maxLog2TrDynamicRange;
  const int shift_2nd = log2i( n ) + TRANSFORM_MATRIX_SHIFT;

  // Horizontal pass: transform along rows, writing transposed output.
  switch ( n )
  {
  case 4:
    if ( type == DCT2 )
      fastForwardDCT2_B4( block, tmp, shift_1st, n, 0, 0 );
    else if ( type == DCT8 )
      fastForwardDCT8_B4( block, tmp, shift_1st, n, 0, 0 );
    else
      fastForwardDST7_B4( block, tmp, shift_1st, n, 0, 0 );
    break;
  case 8:
    if ( type == DCT2 )
      fastForwardDCT2_B8( block, tmp, shift_1st, n, 0, 0 );
    else if ( type == DCT8 )
      fastForwardDCT8_B8( block, tmp, shift_1st, n, 0, 0 );
    else
      fastForwardDST7_B8( block, tmp, shift_1st, n, 0, 0 );
    break;
  case 16:
    if ( type == DCT2 )
      fastForwardDCT2_B16( block, tmp, shift_1st, n, 0, 0 );
    else if ( type == DCT8 )
      fastForwardDCT8_B16( block, tmp, shift_1st, n, 0, 0 );
    else
      fastForwardDST7_B16( block, tmp, shift_1st, n, 0, 0 );
    break;
  case 32:
    if ( type == DCT2 )
      fastForwardDCT2_B32( block, tmp, shift_1st, n, 0, 0 );
    else if ( type == DCT8 )
      fastForwardDCT8_B32( block, tmp, shift_1st, n, 0, 0 );
    else
      fastForwardDST7_B32( block, tmp, shift_1st, n, 0, 0 );
    break;
  default:
    fprintf( stderr, "unsupported size %d\n", n );
    exit( 1 );
  }

  // Vertical pass.
  switch ( n )
  {
  case 4:
    if ( type == DCT2 )
      fastForwardDCT2_B4( tmp, dst, shift_2nd, n, 0, 0 );
    else if ( type == DCT8 )
      fastForwardDCT8_B4( tmp, dst, shift_2nd, n, 0, 0 );
    else
      fastForwardDST7_B4( tmp, dst, shift_2nd, n, 0, 0 );
    break;
  case 8:
    if ( type == DCT2 )
      fastForwardDCT2_B8( tmp, dst, shift_2nd, n, 0, 0 );
    else if ( type == DCT8 )
      fastForwardDCT8_B8( tmp, dst, shift_2nd, n, 0, 0 );
    else
      fastForwardDST7_B8( tmp, dst, shift_2nd, n, 0, 0 );
    break;
  case 16:
    if ( type == DCT2 )
      fastForwardDCT2_B16( tmp, dst, shift_2nd, n, 0, 0 );
    else if ( type == DCT8 )
      fastForwardDCT8_B16( tmp, dst, shift_2nd, n, 0, 0 );
    else
      fastForwardDST7_B16( tmp, dst, shift_2nd, n, 0, 0 );
    break;
  case 32:
    if ( type == DCT2 )
      fastForwardDCT2_B32( tmp, dst, shift_2nd, n, 0, 0 );
    else if ( type == DCT8 )
      fastForwardDCT8_B32( tmp, dst, shift_2nd, n, 0, 0 );
    else
      fastForwardDST7_B32( tmp, dst, shift_2nd, n, 0, 0 );
    break;
  default:
    exit( 1 );
  }
}

// Run the forward transform with a different matrix per axis, matching
// TrQuant::xT with a multiple transform selection mode.
static void forward_pair(int type_hor, int type_ver, int n, const coeff_t *src, coeff_t *dst)
{
  coeff_t block[MAX_N * MAX_N];
  coeff_t tmp[MAX_N * MAX_N];
  memcpy(block, src, sizeof(coeff_t) * n * n);

  const int shift_1st = ( log2i( n ) + bitDepth + TRANSFORM_MATRIX_SHIFT ) - maxLog2TrDynamicRange;
  const int shift_2nd = log2i( n ) + TRANSFORM_MATRIX_SHIFT;

#define FWD_PICK( fn )                       \
  switch ( n )                               \
  {                                          \
  case 4:  fn##_B4 ( block, tmp, shift_1st, n, 0, 0 ); break;   \
  case 8:  fn##_B8 ( block, tmp, shift_1st, n, 0, 0 ); break;   \
  case 16: fn##_B16( block, tmp, shift_1st, n, 0, 0 ); break;   \
  case 32: fn##_B32( block, tmp, shift_1st, n, 0, 0 ); break;   \
  default: exit( 1 );                        \
  }

  if ( type_hor == DCT2 ) FWD_PICK( fastForwardDCT2 )
  else if ( type_hor == DCT8 ) FWD_PICK( fastForwardDCT8 )
  else FWD_PICK( fastForwardDST7 )

#undef FWD_PICK

#define FWD_PICK2( fn )                      \
  switch ( n )                               \
  {                                          \
  case 4:  fn##_B4 ( tmp, dst, shift_2nd, n, 0, 0 ); break;   \
  case 8:  fn##_B8 ( tmp, dst, shift_2nd, n, 0, 0 ); break;   \
  case 16: fn##_B16( tmp, dst, shift_2nd, n, 0, 0 ); break;   \
  case 32: fn##_B32( tmp, dst, shift_2nd, n, 0, 0 ); break;   \
  default: exit( 1 );                        \
  }

  if ( type_ver == DCT2 ) FWD_PICK2( fastForwardDCT2 )
  else if ( type_ver == DCT8 ) FWD_PICK2( fastForwardDCT8 )
  else FWD_PICK2( fastForwardDST7 )

#undef FWD_PICK2
}

// Run the inverse transform with a different matrix per axis, matching
// TrQuant::xIT with a multiple transform selection mode.
static void inverse_pair(int type_hor, int type_ver, int n, const coeff_t *src, coeff_t *dst)
{
  coeff_t tmp[MAX_N * MAX_N];
  const int shift_1st = TRANSFORM_MATRIX_SHIFT + 1;
  const int shift_2nd = ( TRANSFORM_MATRIX_SHIFT + maxLog2TrDynamicRange - 1 ) - bitDepth;
  const coeff_t clip_min = -( 1 << maxLog2TrDynamicRange );
  const coeff_t clip_max = ( 1 << maxLog2TrDynamicRange ) - 1;

#define INV_PICK( fn )                                            \
  switch ( n )                                                   \
  {                                                              \
  case 4:  fn##_B4 ( src, tmp, shift_1st, n, 0, 0, clip_min, clip_max ); break;   \
  case 8:  fn##_B8 ( src, tmp, shift_1st, n, 0, 0, clip_min, clip_max ); break;   \
  case 16: fn##_B16( src, tmp, shift_1st, n, 0, 0, clip_min, clip_max ); break;   \
  case 32: fn##_B32( src, tmp, shift_1st, n, 0, 0, clip_min, clip_max ); break;   \
  default: exit( 1 );                                            \
  }

  if ( type_ver == DCT2 ) INV_PICK( fastInverseDCT2 )
  else if ( type_ver == DCT8 ) INV_PICK( fastInverseDCT8 )
  else INV_PICK( fastInverseDST7 )

#undef INV_PICK

#define INV_PICK2( fn)                                           \
  switch ( n )                                                   \
  {                                                              \
  case 4:  fn##_B4 ( tmp, dst, shift_2nd, n, 0, 0, clip_min, clip_max ); break;   \
  case 8:  fn##_B8 ( tmp, dst, shift_2nd, n, 0, 0, clip_min, clip_max ); break;   \
  case 16: fn##_B16( tmp, dst, shift_2nd, n, 0, 0, clip_min, clip_max ); break;   \
  case 32: fn##_B32( tmp, dst, shift_2nd, n, 0, 0, clip_min, clip_max ); break;   \
  default: exit( 1 );                                            \
  }

  if ( type_hor == DCT2 ) INV_PICK2( fastInverseDCT2 )
  else if ( type_hor == DCT8 ) INV_PICK2( fastInverseDCT8 )
  else INV_PICK2( fastInverseDST7 )

#undef INV_PICK2
}

// Run the inverse transform of the given type over an n x n coefficient
// block, using the same structure and shifts as TrQuant::xIT.
static void inverse(int type, int n, const coeff_t *src, coeff_t *dst)
{
  coeff_t tmp[MAX_N * MAX_N];
  const int shift_1st = TRANSFORM_MATRIX_SHIFT + 1;
  const int shift_2nd = ( TRANSFORM_MATRIX_SHIFT + maxLog2TrDynamicRange - 1 ) - bitDepth;
  const coeff_t clip_min = -( 1 << maxLog2TrDynamicRange );
  const coeff_t clip_max = ( 1 << maxLog2TrDynamicRange ) - 1;

  // Vertical pass first.
  switch ( n )
  {
  case 4:
    if ( type == DCT2 )
      fastInverseDCT2_B4( src, tmp, shift_1st, n, 0, 0, clip_min, clip_max );
    else if ( type == DCT8 )
      fastInverseDCT8_B4( src, tmp, shift_1st, n, 0, 0, clip_min, clip_max );
    else
      fastInverseDST7_B4( src, tmp, shift_1st, n, 0, 0, clip_min, clip_max );
    break;
  case 8:
    if ( type == DCT2 )
      fastInverseDCT2_B8( src, tmp, shift_1st, n, 0, 0, clip_min, clip_max );
    else if ( type == DCT8 )
      fastInverseDCT8_B8( src, tmp, shift_1st, n, 0, 0, clip_min, clip_max );
    else
      fastInverseDST7_B8( src, tmp, shift_1st, n, 0, 0, clip_min, clip_max );
    break;
  case 16:
    if ( type == DCT2 )
      fastInverseDCT2_B16( src, tmp, shift_1st, n, 0, 0, clip_min, clip_max );
    else if ( type == DCT8 )
      fastInverseDCT8_B16( src, tmp, shift_1st, n, 0, 0, clip_min, clip_max );
    else
      fastInverseDST7_B16( src, tmp, shift_1st, n, 0, 0, clip_min, clip_max );
    break;
  case 32:
    if ( type == DCT2 )
      fastInverseDCT2_B32( src, tmp, shift_1st, n, 0, 0, clip_min, clip_max );
    else if ( type == DCT8 )
      fastInverseDCT8_B32( src, tmp, shift_1st, n, 0, 0, clip_min, clip_max );
    else
      fastInverseDST7_B32( src, tmp, shift_1st, n, 0, 0, clip_min, clip_max );
    break;
  default:
    exit( 1 );
  }

  // Horizontal pass.
  switch ( n )
  {
  case 4:
    if ( type == DCT2 )
      fastInverseDCT2_B4( tmp, dst, shift_2nd, n, 0, 0, clip_min, clip_max );
    else if ( type == DCT8 )
      fastInverseDCT8_B4( tmp, dst, shift_2nd, n, 0, 0, clip_min, clip_max );
    else
      fastInverseDST7_B4( tmp, dst, shift_2nd, n, 0, 0, clip_min, clip_max );
    break;
  case 8:
    if ( type == DCT2 )
      fastInverseDCT2_B8( tmp, dst, shift_2nd, n, 0, 0, clip_min, clip_max );
    else if ( type == DCT8 )
      fastInverseDCT8_B8( tmp, dst, shift_2nd, n, 0, 0, clip_min, clip_max );
    else
      fastInverseDST7_B8( tmp, dst, shift_2nd, n, 0, 0, clip_min, clip_max );
    break;
  case 16:
    if ( type == DCT2 )
      fastInverseDCT2_B16( tmp, dst, shift_2nd, n, 0, 0, clip_min, clip_max );
    else if ( type == DCT8 )
      fastInverseDCT8_B16( tmp, dst, shift_2nd, n, 0, 0, clip_min, clip_max );
    else
      fastInverseDST7_B16( tmp, dst, shift_2nd, n, 0, 0, clip_min, clip_max );
    break;
  case 32:
    if ( type == DCT2 )
      fastInverseDCT2_B32( tmp, dst, shift_2nd, n, 0, 0, clip_min, clip_max );
    else if ( type == DCT8 )
      fastInverseDCT8_B32( tmp, dst, shift_2nd, n, 0, 0, clip_min, clip_max );
    else
      fastInverseDST7_B32( tmp, dst, shift_2nd, n, 0, 0, clip_min, clip_max );
    break;
  default:
    exit( 1 );
  }
}

// A deterministic pseudo-random block, so the vectors are reproducible.
static void fill_block(coeff_t *block, int n, unsigned seed, int amplitude)
{
  unsigned state = seed;
  for ( int i = 0; i < n * n; i++ )
  {
    state = state * 1103515245u + 12345u;
    block[i] = (int)( ( state >> 16 ) & 0x7fff ) % ( 2 * amplitude + 1 ) - amplitude;
  }
}

static void print_block(const char *label, const coeff_t *block, int n)
{
  printf( "    %s: [%s", label, "" );
  for ( int i = 0; i < n * n; i++ )
  {
    printf( "%s%d", i ? ", " : "", block[i]);
  }
  printf( "],\n" );
}

int main(void)
{
  const int sizes[]  = { 4, 8, 16, 32 };
  const int n_sizes  = 4;
  const char *names[] = { "Dct2", "Dct8", "Dst7" };
  const int types[]  = { DCT2, DCT8, DST7 };

  printf( "// VVC integer transform reference vectors.\n" );
  printf( "//\n" );
  printf( "// Generated by `tools/transform_oracle.c`, which links Fraunhofer HHI\n" );
  printf( "// VVenC's own transform kernels. `girc-core` asserts bit-exact equality\n" );
  printf( "// against these vectors, so the reference implementation is the oracle.\n" );
  printf( "//\n" );
  printf( "// Do not edit by hand; regenerate with `tools/generate_transform_oracle.sh`.\n" );
  printf( "//\n" );
  printf( "//! Reference vectors from the VVC reference implementation.\n" );
  printf( "//\n" );
  printf( "//! Auto-generated; see `tools/transform_oracle.c`.\n" );
  printf( "\n" );
  printf( "pub struct OracleVector {\n" );
  printf( "    pub kind: usize,\n" );
  printf( "    pub size: usize,\n" );
  printf( "    pub input: &'static [i32],\n" );
  printf( "    pub forward: &'static [i32],\n" );
  printf( "    pub inverse: &'static [i32],\n" );
  printf( "}\n" );
  printf( "\n" );
  printf( "/// A reference vector for a multiple transform selection mode.\n" );
  printf( "///\n" );
  printf( "/// The mode is the pair of per-axis transforms, in the order the\n" );
  printf( "/// standard numbers the modes.\n" );
  printf( "pub struct OracleModeVector {\n" );
  printf( "    pub horizontal: usize,\n" );
  printf( "    pub vertical: usize,\n" );
  printf( "    pub size: usize,\n" );
  printf( "    pub input: &'static [i32],\n" );
  printf( "    pub forward: &'static [i32],\n" );
  printf( "    pub inverse: &'static [i32],\n" );
  printf( "}\n" );
  printf( "\n" );
  printf( "pub static ORACLE_VECTORS: &[OracleVector] = &[\n" );

  for ( int s = 0; s < n_sizes; s++ )
  {
    const int n = sizes[s];
    for ( int t = 0; t < 3; t++ )
    {
      coeff_t in[MAX_N * MAX_N];
      coeff_t fwd[MAX_N * MAX_N];
      coeff_t inv[MAX_N * MAX_N];
      fill_block( in, n, 0xC0FFEEu + (unsigned)( s * 3 + t ), 64 );

      forward( types[t], n, in, fwd );
      inverse( types[t], n, fwd, inv );

      printf( "    OracleVector {\n" );
      printf( "        kind: %d,\n", t );
      printf( "        size: %d,\n", n );
      printf( "        input: &[" );
      for ( int i = 0; i < n * n; i++ )
        printf( "%s%d", i ? ", " : "", in[i]);
      printf( "],\n" );
      printf( "        forward: &[" );
      for ( int i = 0; i < n * n; i++ )
        printf( "%s%d", i ? ", " : "", fwd[i]);
      printf( "],\n" );
      printf( "        inverse: &[" );
      for ( int i = 0; i < n * n; i++ )
        printf( "%s%d", i ? ", " : "", inv[i]);
      printf( "],\n" );
      printf( "    },\n" );
    }
  }

  printf( "];\n" );

  printf( "\n" );
  printf( "pub static ORACLE_MODE_VECTORS: &[OracleModeVector] = &[\n" );

  // The four multiple transform selection modes, in the order the
  // standard numbers them.
  const int mode_hor[] = { DST7, DCT8, DST7, DCT8 };
  const int mode_ver[] = { DST7, DST7, DCT8, DCT8 };

  for ( int s = 0; s < n_sizes; s++ )
  {
    const int n = sizes[s];
    for ( int m = 0; m < 4; m++ )
    {
      coeff_t in[MAX_N * MAX_N];
      coeff_t fwd[MAX_N * MAX_N];
      coeff_t inv[MAX_N * MAX_N];
      fill_block( in, n, 0x5EED5EEDu + (unsigned)( s * 4 + m ), 64 );

      forward_pair( mode_hor[m], mode_ver[m], n, in, fwd );
      inverse_pair( mode_hor[m], mode_ver[m], n, fwd, inv );

      printf( "    OracleModeVector {\n" );
      printf( "        horizontal: %d,\n", mode_hor[m] == DCT2 ? 0 : ( mode_hor[m] == DCT8 ? 1 : 2 ) );
      printf( "        vertical: %d,\n", mode_ver[m] == DCT2 ? 0 : ( mode_ver[m] == DCT8 ? 1 : 2 ) );
      printf( "        size: %d,\n", n );
      printf( "        input: &[" );
      for ( int i = 0; i < n * n; i++ )
        printf( "%s%d", i ? ", " : "", in[i] );
      printf( "],\n" );
      printf( "        forward: &[" );
      for ( int i = 0; i < n * n; i++ )
        printf( "%s%d", i ? ", " : "", fwd[i] );
      printf( "],\n" );
      printf( "        inverse: &[" );
      for ( int i = 0; i < n * n; i++ )
        printf( "%s%d", i ? ", " : "", inv[i] );
      printf( "],\n" );
      printf( "    },\n" );
    }
  }

  printf( "];\n" );
  return 0;
}
