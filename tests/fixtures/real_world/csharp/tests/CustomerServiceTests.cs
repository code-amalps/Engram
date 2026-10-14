using Commerce.Application.Services;
using Commerce.Domain.Entities;
using Commerce.Domain.Interfaces;

namespace Commerce.Tests;

public class FakeCustomerRepository : ICustomerRepository
{
    private readonly Dictionary<string, Customer> _store = new();
    public Customer? GetById(string id) => _store.GetValueOrDefault(id);
    public void Save(Customer customer) => _store[customer.Id] = customer;
    public IEnumerable<Customer> GetAllActive() => _store.Values.Where(c => c.IsActive);
}

public class CustomerServiceTests
{
    public void TestRegistration()
    {
        var repo = new FakeCustomerRepository();
        var svc = new CustomerService(repo);
        var cust = svc.RegisterCustomer("c1", "Alice", "alice@example.com");
        if (cust.FullName != "Alice") throw new Exception("Mismatch");
    }
}
