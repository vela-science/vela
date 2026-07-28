#include <algorithm>
#include <cstdint>
#include <fstream>
#include <iostream>
#include <limits>
#include <sstream>
#include <string>
#include <vector>

namespace {

#ifndef CANOPUS_RANGE_START
#define CANOPUS_RANGE_START 10428008
#endif

#ifndef CANOPUS_RANGE_END
#define CANOPUS_RANGE_END 10428200
#endif

constexpr std::uint32_t kRangeStart = CANOPUS_RANGE_START;
constexpr std::uint32_t kRangeEnd = CANOPUS_RANGE_END;
constexpr std::uint8_t kRequiredCuts = 16;
constexpr std::size_t kTableSize = 1u << 25;
constexpr std::size_t kTableMask = kTableSize - 1;

struct Result {
  bool witness = false;
  std::uint32_t primes_tested = 0;
  std::uint8_t max_multiplicity = 0;
  std::uint32_t best_p = 0;
  std::uint32_t best_residue = 0;
  std::vector<std::uint32_t> cuts;
};

bool is_prime(std::uint32_t n) {
  if (n < 2) return false;
  if ((n & 1u) == 0) return n == 2;
  for (std::uint32_t d = 3; static_cast<std::uint64_t>(d) * d <= n; d += 2) {
    if (n % d == 0) return false;
  }
  return true;
}

std::size_t slot_for(std::uint32_t key) {
  std::uint64_t mixed = static_cast<std::uint64_t>(key) * 11400714819323198485ull;
  return static_cast<std::size_t>(mixed >> (64 - 25));
}

Result search() {
  std::vector<std::uint32_t> keys(kTableSize);
  std::vector<std::uint32_t> generations(kTableSize);
  std::vector<std::uint8_t> counts(kTableSize);
  std::vector<std::uint32_t> used;
  used.reserve(kRangeEnd);
  std::uint32_t generation = 0;
  Result result;

  for (std::uint32_t p = kRangeStart; p <= kRangeEnd; ++p) {
    if (!is_prime(p)) continue;
    ++result.primes_tested;
    ++generation;
    used.clear();
    std::uint64_t factorial = 1;

    for (std::uint32_t cut = 0; cut < p; ++cut) {
      if (cut > 0) factorial = (factorial * cut) % p;
      const auto residue = static_cast<std::uint32_t>(factorial);
      std::size_t slot = slot_for(residue);
      while (generations[slot] == generation && keys[slot] != residue) {
        slot = (slot + 1) & kTableMask;
      }
      if (generations[slot] != generation) {
        generations[slot] = generation;
        keys[slot] = residue;
        counts[slot] = 1;
        used.push_back(static_cast<std::uint32_t>(slot));
      } else if (counts[slot] != std::numeric_limits<std::uint8_t>::max()) {
        ++counts[slot];
      }
    }

    std::uint8_t local_max = 0;
    std::uint32_t local_residue = 0;
    for (const auto raw_slot : used) {
      const auto slot = static_cast<std::size_t>(raw_slot);
      if (counts[slot] > local_max ||
          (counts[slot] == local_max && keys[slot] < local_residue)) {
        local_max = counts[slot];
        local_residue = keys[slot];
      }
    }
    if (local_max > result.max_multiplicity) {
      result.max_multiplicity = local_max;
      result.best_p = p;
      result.best_residue = local_residue;
    }
    if (local_max >= kRequiredCuts) {
      result.witness = true;
      result.max_multiplicity = local_max;
      result.best_p = p;
      result.best_residue = local_residue;
      break;
    }
  }

  std::uint64_t factorial = 1;
  for (std::uint32_t cut = 0; cut < result.best_p; ++cut) {
    if (cut > 0) factorial = (factorial * cut) % result.best_p;
    if (factorial == result.best_residue) result.cuts.push_back(cut);
  }
  return result;
}

std::string canonical_artifact(const Result& result) {
  std::ostringstream out;
  out << "schema=canopus.erdos1056-k15-search.v1\n";
  out << "status=" << (result.witness ? "witness" : "negative") << "\n";
  out << "problem=1056\n";
  out << "k=15\n";
  out << "range_start=" << kRangeStart << "\n";
  out << "range_end=" << kRangeEnd << "\n";
  out << "primes_tested=" << result.primes_tested << "\n";
  out << "max_multiplicity=" << static_cast<unsigned>(result.max_multiplicity) << "\n";
  out << "best_p=" << result.best_p << "\n";
  out << "best_residue=" << result.best_residue << "\n";
  out << "cuts=";
  for (std::size_t i = 0; i < result.cuts.size(); ++i) {
    if (i != 0) out << ',';
    out << result.cuts[i];
  }
  out << '\n';
  return out.str();
}

std::string read_exact(const char* path) {
  std::ifstream input(path, std::ios::binary);
  if (!input) throw std::runtime_error("cannot open candidate artifact");
  std::ostringstream bytes;
  bytes << input.rdbuf();
  if (!input.good() && !input.eof()) throw std::runtime_error("cannot read candidate artifact");
  const auto value = bytes.str();
  if (value.size() > 65536) throw std::runtime_error("candidate artifact is oversized");
  return value;
}

}  // namespace

int main(int argc, char** argv) {
  try {
    if (argc != 2) {
      std::cerr << "usage: verifier ARTIFACT\n";
      return 64;
    }
    const auto result = search();
    const auto expected = canonical_artifact(result);
    const auto observed = read_exact(argv[1]);
    if (observed != expected) {
      std::cerr << "candidate does not match the independently recomputed bounded search\n";
      return 1;
    }
    std::cout << "verified erdos:1056 k=15 "
              << (result.witness ? "witness" : "bounded-negative")
              << " primes=" << result.primes_tested
              << " max_multiplicity=" << static_cast<unsigned>(result.max_multiplicity)
              << "\n";
    return 0;
  } catch (const std::exception& error) {
    std::cerr << error.what() << '\n';
    return 70;
  }
}
