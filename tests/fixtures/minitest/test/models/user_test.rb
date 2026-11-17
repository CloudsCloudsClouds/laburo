require "test_helper"

class UserTest < ActiveSupport::TestCase
  test "valid user" do
    assert User.new(name: "Ada").valid?
  end

  def test_rejects_blank_name
    refute User.new(name: "").valid?
  end
end
